//! Explicit local Spring Boot plugin declarations; no effective-POM or Gradle execution.
use super::metadata::read_manifest;
use anyhow::Result;
use std::path::Path;

pub fn build_tools(root: &Path) -> Result<Vec<&'static str>> {
    let mut result = Vec::new();
    if root.join("pom.xml").is_file() {
        let text = read_manifest(&root.join("pom.xml"))?;
        let xml = roxmltree::Document::parse(&text)?;
        let plugin = xml.descendants().any(|node| {
            node.has_tag_name("plugin")
                && node.ancestors().any(|p| p.has_tag_name("build"))
                && !node
                    .ancestors()
                    .any(|p| p.has_tag_name("pluginManagement") || p.has_tag_name("profile"))
                && node.children().any(|child| {
                    child.has_tag_name("artifactId")
                        && child.text().map(str::trim) == Some("spring-boot-maven-plugin")
                })
                && node.children().any(|child| {
                    child.has_tag_name("groupId")
                        && child.text().map(str::trim) == Some("org.springframework.boot")
                })
        });
        if plugin {
            result.push("maven");
        }
    }
    for file in ["build.gradle", "build.gradle.kts"] {
        if !root.join(file).is_file() {
            continue;
        }
        if gradle_boot_plugin(&read_manifest(&root.join(file))?) && !result.contains(&"gradle") {
            result.push("gradle");
        }
    }
    Ok(result)
}

#[derive(PartialEq)]
enum Token {
    Word(String),
    Literal(String),
    Symbol(char),
}

/// Tokens keep strings opaque, so comments and example text do not look like plugins.
fn gradle_boot_plugin(text: &str) -> bool {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else if c == '\'' || c == '"' {
            let mut value = String::new();
            while let Some(next) = chars.next() {
                if next == c {
                    break;
                }
                if next == '\\' {
                    if let Some(next) = chars.next() {
                        value.push(next);
                    }
                } else {
                    value.push(next);
                }
            }
            tokens.push(Token::Literal(value));
        } else if c.is_alphabetic() || c == '_' {
            let mut word = String::from(c);
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                word.push(chars.next().unwrap());
            }
            tokens.push(Token::Word(word));
        } else if !c.is_whitespace() {
            tokens.push(Token::Symbol(c));
        }
    }
    let word = |token: Option<&Token>, expected: &str| matches!(token, Some(Token::Word(value)) if value == expected);
    let literal = |token: Option<&Token>| matches!(token, Some(Token::Literal(value)) if value == "org.springframework.boot");
    for i in 0..tokens.len() {
        if word(tokens.get(i), "apply")
            && word(tokens.get(i + 1), "plugin")
            && tokens.get(i + 2) == Some(&Token::Symbol(':'))
            && literal(tokens.get(i + 3))
        {
            return true;
        }
        if !word(tokens.get(i), "id") {
            continue;
        }
        let name = i + if tokens.get(i + 1) == Some(&Token::Symbol('(')) {
            2
        } else {
            1
        };
        if !literal(tokens.get(name)) {
            continue;
        }
        let tail: Vec<_> = tokens
            .iter()
            .skip(name + 1)
            .take_while(|t| {
                !matches!(t, Token::Symbol('}') | Token::Symbol(';')) && !word(Some(t), "id")
            })
            .collect();
        let disabled = tail.iter().enumerate().any(|(index, token)| {
            word(Some(token), "apply")
                && (word(tail.get(index + 1).copied(), "false")
                    || (tail.get(index + 1).copied() == Some(&Token::Symbol('('))
                        && word(tail.get(index + 2).copied(), "false")))
        });
        if !disabled {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_enabled_plugins_produce_a_boot_capability() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("pom.xml"), "<project><build><plugins><plugin><groupId>org.springframework.boot</groupId><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build></project>").unwrap();
        assert_eq!(build_tools(root).unwrap(), ["maven"]);
        std::fs::write(
            root.join("pom.xml"),
            "<project><!-- spring-boot-maven-plugin --></project>",
        )
        .unwrap();
        std::fs::write(
            root.join("build.gradle.kts"),
            "// id(\"org.springframework.boot\")\nplugins { id(\"java\") }\n",
        )
        .unwrap();
        assert!(build_tools(root).unwrap().is_empty());
        std::fs::write(
            root.join("build.gradle.kts"),
            "plugins { id(\"org.springframework.boot\") version \"3.5.0\" }\n",
        )
        .unwrap();
        assert_eq!(build_tools(root).unwrap(), ["gradle"]);
        for text in [
            "plugins { id(\"org.springframework.boot\") version \"3.5.0\" apply false }",
            "plugins { id(\"org.springframework.boot\").version(\"3.5.0\").apply(false) }",
            "println(\"id('org.springframework.boot')\")",
        ] {
            std::fs::write(root.join("build.gradle.kts"), text).unwrap();
            assert!(build_tools(root).unwrap().is_empty());
        }
    }
}
