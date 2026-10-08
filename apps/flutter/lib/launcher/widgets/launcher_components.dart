import 'package:flutter/material.dart';
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
      this.onBack,
      this.trailing});
  final LauncherViewState state;
  final TextEditingController input;
  final FocusNode focus;
  final VoidCallback? onBack;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => SizedBox(
      height: state.inputAreaHeight,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Row(children: [
            if (onBack != null)
              IconButton(
                  tooltip: 'Back to results',
                  onPressed: onBack,
                  icon: const Icon(AppIcons.back, size: 23))
            else
              AppIconPlate(
                  state.isContent
                      ? (state.contentSource == ContextSourceDto.selectedText
                          ? AppIcons.quote
                          : AppIcons.clipboard)
                      : AppIcons.search,
                  size: 40),
            const SizedBox(width: 12),
            Expanded(
                child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                  if (state.isContent)
                    Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: Text('${state.contentLabel} · Esc → Current App',
                            key: const Key('captured-content-label'),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                                color: AppStyle.secondary))),
                  TextField(
                      controller: input,
                      focusNode: focus,
                      autofocus: true,
                      maxLines: state.isContent ? 2 : 1,
                      style: TextStyle(
                          fontSize: state.isContent ? 17 : 23,
                          height: 1.25,
                          fontWeight: FontWeight.w400,
                          color: AppStyle.primary),
                      cursorColor: AppStyle.primary,
                      cursorWidth: 1.4,
                      decoration: InputDecoration(
                          hintText: state.searchHint,
                          hintStyle: const TextStyle(
                              color: AppStyle.secondary,
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
              trailing ?? const AppKeycap('esc'),
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
  Widget build(BuildContext context) => SizedBox(
      height: LauncherViewState.scopeHeight,
      child: ListView(
          key: const Key('scope-bar'),
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          children: [
            for (final scope in scopes)
              Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 3, vertical: 5),
                  child: Semantics(
                      selected: scope.id == selectedId,
                      button: true,
                      child: Tooltip(
                          message: scope.isCurrentApp
                              ? 'All actions available in the current app'
                              : 'Filter ${scope.label} actions',
                          child: Material(
                              color: scope.id == selectedId
                                  ? AppStyle.buttonSelection
                                  : Colors.transparent,
                              shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(10),
                                  side: BorderSide(
                                      color: scope.id == selectedId
                                          ? AppStyle.controlBorder
                                          : Colors.transparent,
                                      width: .6)),
                              clipBehavior: Clip.antiAlias,
                              child: InkWell(
                                  borderRadius: BorderRadius.circular(10),
                                  onTap: () => onSelect(scope.id),
                                  child: Padding(
                                      padding: const EdgeInsets.symmetric(
                                          horizontal: 12),
                                      child: Center(
                                          child: Text(scope.label,
                                              style:
                                                  TextStyle(fontSize: 12, fontWeight: scope.id == selectedId ? FontWeight.w600 : FontWeight.w400, color: scope.id == selectedId ? AppStyle.primary : AppStyle.secondary))))))))),
          ]));
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

class LauncherFooter extends StatelessWidget {
  const LauncherFooter(
      {super.key,
      required this.label,
      required this.primaryLabel,
      required this.onPrimary,
      this.onCopy,
      this.onBack,
      this.result = false});
  final String label, primaryLabel;
  final VoidCallback? onPrimary, onCopy, onBack;
  final bool result;
  @override
  Widget build(BuildContext context) => SizedBox(
      key: const Key('launcher-footer'),
      height: LauncherViewState.footerHeight,
      child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 6, 24, 10),
          child: Row(children: [
            Text(label,
                style: const TextStyle(fontSize: 11, color: AppStyle.muted)),
            const Spacer(),
            if (!result) ...[
              const Text('↑↓  Navigate',
                  style: TextStyle(fontSize: 11, color: AppStyle.muted)),
              const SizedBox(width: 14),
            ],
            if (onCopy != null)
              TextButton(onPressed: onCopy, child: const Text('Copy')),
            if (onBack != null)
              TextButton(onPressed: onBack, child: const Text('Back')),
            if (!result)
              OutlinedButton(
                  onPressed: onPrimary,
                  style: OutlinedButton.styleFrom(
                      backgroundColor: AppStyle.actionSurface,
                      foregroundColor: AppStyle.primary,
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      minimumSize: const Size(0, 24)),
                  child: Text(primaryLabel,
                      style: const TextStyle(
                          fontSize: 11, fontWeight: FontWeight.w500))),
            const SizedBox(width: 4),
            AppKeycap(result ? 'esc' : '↵'),
          ])));
}

class MenuNavigationRow extends StatelessWidget {
  const MenuNavigationRow(
      {super.key,
      required this.title,
      required this.selected,
      required this.onTap});
  final String title;
  final bool selected;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Semantics(
      button: true,
      selected: selected,
      child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 2),
          child: Material(
              color: selected ? AppStyle.selectedSurface : Colors.transparent,
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(AppStyle.resultRadius),
                  side: BorderSide(
                      color: selected
                          ? AppStyle.selectionOutline
                          : Colors.transparent,
                      width: .5)),
              child: InkWell(
                  borderRadius: BorderRadius.circular(AppStyle.resultRadius),
                  onTap: onTap,
                  child: SizedBox(
                      height: LauncherViewState.rowHeight - 4,
                      child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 10),
                          child: Row(children: [
                            SizedBox(
                                width: 36,
                                height: 36,
                                child: const Icon(AppIcons.folder,
                                    size: 22, color: Color(0xff92c9f6))),
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
                            const Icon(AppIcons.chevron,
                                size: 14, color: AppStyle.secondary),
                          ])))))));
}
