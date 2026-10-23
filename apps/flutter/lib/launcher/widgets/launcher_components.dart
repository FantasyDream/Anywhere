import 'package:flutter/material.dart';

import '../../l10n/app_language.dart';
import '../../src/rust/api.dart';
import '../../ui/app_components.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';
import '../launcher_view_state.dart';

/// Search stays visually dominant; captured content identifies its source here.
class LauncherSearchArea extends StatelessWidget {
  const LauncherSearchArea(
      {super.key,
      required this.state,
      required this.input,
      required this.focus,
      this.onToggleNavigation,
      this.onBack});
  final LauncherViewState state;
  final TextEditingController input;
  final FocusNode focus;
  final VoidCallback? onToggleNavigation;
  final VoidCallback? onBack;

  String _fileRangeLabel(BuildContext context) {
    final roots = state.snapshot?.fileSearch?.roots ?? const <String>[];
    if (roots.isEmpty) return context.l10n.fileSearch;
    if (roots.length > 1) return context.l10n.fileLocations(roots.length);
    final name = roots.single
            .split(RegExp(r'[/\\]'))
            .where((part) => part.isNotEmpty)
            .lastOrNull ??
        roots.single;
    return context.l10n.fileRange(name);
  }

  @override
  Widget build(BuildContext context) => AppPageHeader(
      height: state.inputAreaHeight,
      child: Row(children: [
        if (onBack != null)
          IconButton(
              tooltip: context.l10n.backToResults,
              onPressed: onBack,
              icon: const Icon(AppIcons.back, size: 20))
        else if (!state.isContent && onToggleNavigation != null)
          Semantics(
              button: true,
              expanded: state.navigationOpen,
              label: context.l10n.navigationModes,
              child: Tooltip(
                  message: context.l10n.navigationModes,
                  child: InkWell(
                      key: const Key('launcher-navigation'),
                      borderRadius:
                          BorderRadius.circular(AppStyle.controlRadius),
                      onTap: state.resolving || state.executing
                          ? null
                          : onToggleNavigation,
                      child: SizedBox.square(
                          dimension: 36,
                          child: Row(mainAxisSize: MainAxisSize.min, children: [
                            const Icon(AppIcons.search,
                                size: 24, color: AppStyle.primary),
                            const SizedBox(width: 3),
                            RotatedBox(
                                quarterTurns: state.navigationOpen ? 2 : 0,
                                child: const Icon(AppIcons.chevronDown,
                                    size: 9, color: AppStyle.secondary)),
                          ])))))
        else
          AppIconPlate(
              state.isContent
                  ? (state.contentSource == ContextSourceDto.selectedText
                      ? AppIcons.quote
                      : AppIcons.clipboard)
                  : AppIcons.search,
              size: 36),
        const SizedBox(width: 12),
        Expanded(
            // Both neighboring controls change during an asynchronous Host
            // transition. The parent slot must survive, not just TextField.
            key: const Key('launcher-input-slot'),
            child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (state.isFileSearch)
                    Padding(
                        padding: const EdgeInsets.only(bottom: 3),
                        child: Tooltip(
                            message: state.fileRangeDetail,
                            child: Text(_fileRangeLabel(context),
                                key: const Key('launcher-mode-label'),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: AppStyle.caption.copyWith(
                                    fontSize: DesignTokens.pageModeSize,
                                    fontWeight: DesignTokens.pageModeWeight)))),
                  if (!state.isFileSearch &&
                      !state.isContent &&
                      !state.hasScopeBar &&
                      state.selectedScope?.isCurrentApp == true)
                    Padding(
                        padding: const EdgeInsets.only(bottom: 3),
                        child: Text(state.selectedScope!.label,
                            key: const Key('launcher-mode-label'),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AppStyle.caption)),
                  if (state.isContent)
                    Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: Text(
                            context.l10n.contentEscapeHint(
                                state.contentSource ==
                                        ContextSourceDto.selectedText
                                    ? context.l10n.selectedText
                                    : context.l10n.clipboardText),
                            key: const Key('captured-content-label'),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                                color: AppStyle.secondary))),
                  TextField(
                      // Keep the editing connection alive across host view changes.
                      key: const Key('launcher-search-input'),
                      controller: input,
                      focusNode: focus,
                      autofocus: state.visible,
                      // Launcher commands own Return. A native editing
                      // completion must not close a newer page's input client.
                      textInputAction: TextInputAction.none,
                      onEditingComplete: () {},
                      maxLines: state.isContent ? 2 : 1,
                      style: TextStyle(
                          fontSize: state.isContent
                              ? 17
                              : DesignTokens.pageHeadingSize,
                          height: 1.25,
                          fontWeight: FontWeight.w400,
                          color: AppStyle.primary),
                      cursorColor: AppStyle.primary,
                      cursorWidth: 1.4,
                      decoration: InputDecoration(
                          hintText: state.isFileSearch
                              ? context.l10n.searchFilesFolders
                              : state.isContent
                                  ? context.l10n.editCapturedContent
                                  : state.selectedScope == null
                                      ? context.l10n.searchAppsActions
                                      : context.l10n.searchAppActions(
                                          localizedScopeLabel(
                                              state.selectedScope!,
                                              context.l10n)),
                          hintStyle: const TextStyle(
                              color: AppStyle.muted,
                              fontWeight: FontWeight.w400),
                          filled: false,
                          contentPadding: EdgeInsets.zero,
                          enabledBorder: InputBorder.none,
                          focusedBorder: InputBorder.none,
                          border: InputBorder.none,
                          isDense: true)),
                ])),
        const SizedBox(width: 14),
        if (state.resolving || state.executing)
          const SizedBox.square(
              dimension: 16,
              child: CircularProgressIndicator(
                  strokeWidth: 1.5, color: AppStyle.secondary))
        else
          const AppKeycap('esc'),
      ]));
}

/// Search mode choices live inside the resized glass window, never in an
/// overlay constrained by the one-line empty Launcher.
class LauncherRootNavigation extends StatelessWidget {
  const LauncherRootNavigation(
      {super.key, required this.state, required this.onSelect});
  final LauncherViewState state;
  final ValueChanged<InputModeDto> onSelect;

  @override
  Widget build(BuildContext context) => SizedBox(
      key: const Key('launcher-mode-chooser'),
      height: LauncherViewState.navigationHeight,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Column(children: [
            for (final mode in [
              InputModeDto.search,
              InputModeDto.environment
            ]) ...[
              if (mode == InputModeDto.environment) const SizedBox(height: 4),
              Semantics(
                  button: true,
                  selected: state.navigationMode == mode,
                  child: AppSelectionSurface(
                      key: Key('launcher-navigation-${mode.name}'),
                      selected: state.navigationMode == mode,
                      onTap: () => onSelect(mode),
                      child: SizedBox(
                          height: 44,
                          child: Padding(
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 12),
                              child: Row(children: [
                                Icon(
                                    mode == InputModeDto.search
                                        ? AppIcons.search
                                        : AppIcons.apps,
                                    size: 20,
                                    color: AppStyle.secondary),
                                const SizedBox(width: 14),
                                Expanded(
                                    child: Column(
                                        mainAxisAlignment:
                                            MainAxisAlignment.center,
                                        crossAxisAlignment:
                                            CrossAxisAlignment.start,
                                        children: [
                                      Text(
                                          mode == InputModeDto.search
                                              ? context.l10n.globalSearch
                                              : context.l10n.currentApp,
                                          style: AppStyle.body.copyWith(
                                              fontWeight: FontWeight.w500)),
                                      Text(
                                          mode == InputModeDto.search
                                              ? context.l10n.findAppsAndActions
                                              : context.l10n
                                                  .actionsForYourCurrentEnvironment,
                                          style: AppStyle.caption),
                                    ])),
                                if (state.inputMode == mode)
                                  const Icon(AppIcons.check,
                                      size: 15, color: AppStyle.secondary),
                              ]))))),
            ]
          ])));
}

class LauncherScopeBar extends StatelessWidget {
  const LauncherScopeBar(
      {super.key,
      required this.scopes,
      required this.selectedId,
      required this.onSelect});
  final List<ScopeDto> scopes;
  final String? selectedId;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) => LauncherFilterBar(
      key: const Key('scope-bar'),
      selectedId: selectedId,
      options: [
        for (final scope in scopes)
          LauncherFilterOption(
              scope.id,
              localizedScopeLabel(scope, context.l10n),
              context.l10n.shortcutHint(
                  scope.isCurrentApp
                      ? context.l10n.allActionsAvailableInTheCurrentApp
                      : context.l10n.filterActions(
                          localizedScopeLabel(scope, context.l10n)),
                  'Tab / Shift+Tab')),
      ],
      onSelect: onSelect);
}

class LauncherFilterOption {
  const LauncherFilterOption(this.id, this.label, this.tooltip);
  final String id, label, tooltip;
}

/// One chip bar for Scope and tool filters; keyboard selection stays visible.
class LauncherFilterBar extends StatefulWidget {
  const LauncherFilterBar(
      {super.key,
      required this.options,
      required this.selectedId,
      required this.onSelect,
      this.enabled = true,
      this.keyPrefix = 'filter'});
  final List<LauncherFilterOption> options;
  final String? selectedId;
  final ValueChanged<String> onSelect;
  final bool enabled;
  final String keyPrefix;

  @override
  State<LauncherFilterBar> createState() => _LauncherFilterBarState();
}

class _LauncherFilterBarState extends State<LauncherFilterBar> {
  final _scroll = ScrollController();
  final Map<String, GlobalKey> _keys = {};

  @override
  void initState() {
    super.initState();
    _revealSelection();
  }

  @override
  void didUpdateWidget(LauncherFilterBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.selectedId != widget.selectedId) _revealSelection();
  }

  void _revealSelection() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients) return;
      final selected = _keys[widget.selectedId]?.currentContext;
      if (selected != null) {
        Scrollable.ensureVisible(selected,
            alignment: .5, duration: const Duration(milliseconds: 120));
      }
    });
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ids = widget.options.map((option) => option.id).toSet();
    _keys.removeWhere((id, _) => !ids.contains(id));
    return SizedBox(
        height: LauncherViewState.scopeHeight,
        child: SingleChildScrollView(
            controller: _scroll,
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(children: [
              for (final option in widget.options)
                Padding(
                    key: _keys.putIfAbsent(option.id, () => GlobalKey()),
                    padding:
                        const EdgeInsets.symmetric(horizontal: 3, vertical: 5),
                    child: LauncherFilterButton(
                        key: Key('${widget.keyPrefix}-${option.id}'),
                        label: option.label,
                        selected: option.id == widget.selectedId,
                        tooltip: option.tooltip,
                        onSelect: widget.enabled
                            ? () => widget.onSelect(option.id)
                            : null)),
            ])));
  }
}

/// Shared appearance for scope choices and secondary-page filters.
class LauncherFilterButton extends StatelessWidget {
  const LauncherFilterButton(
      {super.key,
      required this.label,
      required this.selected,
      required this.onSelect,
      this.icon,
      this.tooltip});
  final String label;
  final bool selected;
  final VoidCallback? onSelect;
  final String? tooltip;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final button = Semantics(
        selected: selected,
        button: true,
        child: Material(
            color: selected ? AppStyle.selectedSurface : Colors.transparent,
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(DesignTokens.filterRadius),
                side: BorderSide(
                    color: selected
                        ? AppStyle.selectionOutline
                        : Colors.transparent,
                    width: DesignTokens.dividerWidth)),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
                borderRadius: BorderRadius.circular(DesignTokens.filterRadius),
                hoverColor: selected ? Colors.transparent : AppStyle.hover,
                onTap: onSelect,
                child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Center(
                        child: Row(mainAxisSize: MainAxisSize.min, children: [
                      if (icon != null) ...[
                        Icon(icon,
                            size: 12,
                            color: selected
                                ? AppStyle.primary
                                : AppStyle.secondary),
                        const SizedBox(width: 5)
                      ],
                      Text(label,
                          style: TextStyle(
                              fontSize: DesignTokens.filterSize,
                              fontWeight:
                                  selected ? FontWeight.w600 : FontWeight.w400,
                              color: selected
                                  ? AppStyle.primary
                                  : AppStyle.secondary))
                    ]))))));
    return tooltip == null ? button : Tooltip(message: tooltip!, child: button);
  }
}

/// Retained for Context, menu breadcrumbs and result status; no duplicate app title.
class LauncherResultHeading extends StatelessWidget {
  const LauncherResultHeading({super.key, required this.title, this.detail});
  final String title;
  final String? detail;
  @override
  Widget build(BuildContext context) => SizedBox(
      height: LauncherViewState.headerHeight,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 22),
          child: Row(children: [
            Expanded(
                child: Text(title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w500,
                        color: AppStyle.secondary))),
            if (detail != null) ...[
              const SizedBox(width: 12),
              Flexible(
                  fit: FlexFit.tight,
                  child: Text(detail!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      textAlign: TextAlign.right,
                      style: AppStyle.caption))
            ],
          ])));
}

/// Compact guidance fits the same result region as ordinary rows.
class LauncherEmptyState extends StatelessWidget {
  const LauncherEmptyState(
      {super.key,
      required this.icon,
      required this.title,
      required this.detail});
  final IconData icon;
  final String title, detail;
  @override
  Widget build(BuildContext context) => Center(
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 12),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Icon(icon, size: 28, color: AppStyle.muted),
            const SizedBox(height: 10),
            Flexible(
                child: Text(title,
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style:
                        AppStyle.body.copyWith(fontWeight: FontWeight.w500))),
            const SizedBox(height: 5),
            Flexible(
                child: Tooltip(
                    message: detail,
                    child: Text(detail,
                        textAlign: TextAlign.center,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: AppStyle.caption))),
          ])));
}

class MenuNavigationRow extends StatelessWidget {
  const MenuNavigationRow(
      {super.key,
      required this.title,
      required this.selected,
      required this.onTap,
      this.icon = AppIcons.menu});
  final IconData icon;
  final String title;
  final bool selected;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Semantics(
      button: true,
      selected: selected,
      child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 2),
          child: AppSelectionSurface(
              selected: selected,
              onTap: onTap,
              child: SizedBox(
                  height: LauncherViewState.rowHeight - 4,
                  child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      child: Row(children: [
                        SizedBox(
                            width: 36,
                            height: 36,
                            child: Icon(icon,
                                size: 26,
                                color: icon == AppIcons.folder
                                    ? DesignTokens.iconBlue
                                    : AppStyle.secondary)),
                        const SizedBox(width: 14),
                        Expanded(
                            child: Text(title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                    fontSize: 15,
                                    fontWeight: selected
                                        ? FontWeight.w600
                                        : FontWeight.w500,
                                    color: AppStyle.primary))),
                        if (selected)
                          AppRowActions(
                              primaryLabel: context.l10n.open, onPrimary: onTap)
                        else
                          const Icon(AppIcons.chevron,
                              size: 14, color: AppStyle.secondary),
                      ]))))));
}
