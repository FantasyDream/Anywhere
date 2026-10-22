import '../l10n/app_language.dart';
import '../l10n/context_labels.dart';
import '../src/rust/api.dart';

/// Permission descriptions come from verified types, never package-supplied prose.
String webPermissionLabel(WebPermissionDto permission, AppLocalizations l10n) =>
    switch (permission.kind) {
      WebPermissionKindDto.contextRead => l10n.permissionContextRead(permission
          .contextTypes
          .map((type) => contextTypeLabel(type, l10n))
          .join(', ')),
      WebPermissionKindDto.currentApp => l10n.permissionCurrentApp,
      WebPermissionKindDto.clipboardReadText => l10n.permissionReadClipboard,
      WebPermissionKindDto.clipboardWriteText => l10n.permissionWriteClipboard,
      WebPermissionKindDto.localStorage => l10n.permissionLocalStorage,
    };
