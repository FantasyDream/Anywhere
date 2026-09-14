use crate::{
    command::{CommandDescriptor, CommandRegistry},
    context::ResolvedContext,
};
use launcher_protocol::*;
use std::{collections::HashSet, time::Instant};

#[derive(Debug, Default, Clone, Copy)]
pub struct UsageStats {
    pub execute_count: u32,
    pub last_executed_at: i64,
}

#[derive(Debug, Clone)]
pub struct RankedAction {
    pub presentation: ActionPresentation,
    pub command: CommandDescriptor,
    pub context: ResolvedContext,
}

pub fn rank(
    registry: &CommandRegistry,
    contexts: &[ResolvedContext],
    now: i64,
    usage: impl Fn(&str, &str) -> UsageStats,
) -> Vec<RankedAction> {
    let start = Instant::now();
    let mut actions = Vec::new();
    for context in contexts {
        let source_score = match context.source {
            ContextSource::UserInput => 1000,
            ContextSource::ApplicationContext => 300,
            ContextSource::ForegroundApplication => 0,
        };
        let semantic_score = if context.type_id == TEXT { 100 } else { 400 };
        for command in registry.matching(context) {
            let stats = usage(&command.id, &context.type_id);
            let frequency = stats.execute_count.min(20) as i32 * 2;
            let age = now.saturating_sub(stats.last_executed_at).max(0);
            let recency = if stats.execute_count == 0 {
                0
            } else if age < 3600 {
                20
            } else if age < 86400 {
                10
            } else if age < 604800 {
                5
            } else {
                0
            };
            let score = (semantic_score
                + source_score
                + command.base_priority.clamp(-50, 50)
                + frequency
                + recency) as f32;
            actions.push(RankedAction {
                presentation: ActionPresentation {
                    id: format!(
                        "{}:{}:{}:{}",
                        context.session_id, context.revision, command.id, context.id
                    ),
                    command_id: command.id.clone(),
                    title: command.title.clone(),
                    subtitle: Some(match &command.subtitle {
                        Some(subtitle) => format!("{} · {}", subtitle, context.value.text()),
                        None => context.value.text(),
                    }),
                    icon: IconDescriptor {
                        key: command.icon.clone(),
                    },
                    score,
                },
                command: command.clone(),
                context: context.clone(),
            });
        }
    }
    tracing::info!(count = actions.len(), "commands_matched");
    actions.sort_by(|a, b| {
        b.presentation
            .score
            .total_cmp(&a.presentation.score)
            .then_with(|| a.presentation.command_id.cmp(&b.presentation.command_id))
            .then_with(|| a.context.id.cmp(&b.context.id))
    });
    // The same command on the same object should appear once, using its highest-ranked source.
    let mut seen = HashSet::new();
    actions.retain(|action| {
        seen.insert((
            action.command.id.clone(),
            action.context.type_id.clone(),
            action.context.value.text(),
        ))
    });
    tracing::info!(
        duration_us = start.elapsed().as_micros() as u64,
        count = actions.len(),
        top_command_id = actions.first().map(|a| a.presentation.command_id.as_str()),
        "ranking_completed"
    );
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{context::ContextValue, input::InputEngine};
    #[test]
    fn explicit_url_beats_even_frequent_ambient_directory_and_sort_is_stable() {
        let mut contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        contexts.extend(InputEngine.resolve("https://github.com"));
        let registry = CommandRegistry::builtin();
        let ranked = rank(&registry, &contexts, 1000, |_, kind| {
            if kind == DIRECTORY {
                UsageStats {
                    execute_count: u32::MAX,
                    last_executed_at: 1000,
                }
            } else {
                UsageStats::default()
            }
        });
        assert_eq!(ranked[0].context.type_id, URL);
        assert_eq!(ranked[1].context.type_id, URL);
        let first = rank(&registry, &contexts, 1000, |_, _| UsageStats::default());
        contexts.reverse();
        let second = rank(&registry, &contexts, 1000, |_, _| UsageStats::default());
        assert_eq!(
            first.iter().map(|a| &a.presentation).collect::<Vec<_>>(),
            second.iter().map(|a| &a.presentation).collect::<Vec<_>>()
        );
    }
    #[test]
    fn repeated_command_gets_a_visible_boost() {
        let contexts = InputEngine.resolve("{}");
        let ranked = rank(&CommandRegistry::builtin(), &contexts, 1000, |id, _| {
            if id == "core.json.copy" {
                UsageStats {
                    execute_count: 5,
                    last_executed_at: 1000,
                }
            } else {
                UsageStats::default()
            }
        });
        assert_eq!(ranked[0].command.id, "core.json.copy");
    }
}
