# Permission Class Maintenance 權限類別維護（TipTop p_zw）
# 語系：en。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

class-breadcrumb = System Admin / Permission Class Maintenance
class-title = Permission Class Maintenance
class-list-title = Permission Classes
class-field-code = Permission Class
class-field-name = Description
class-field-menu = Default Menu
class-field-idle-control = Idle Control
class-field-idle-action = Idle Action
class-field-idle-seconds = Idle Time (sec)
class-opt-idle-off = Not controlled
class-opt-idle-class = By permission class
class-opt-idle-system = By system setting
class-opt-idle-warn = Warn only
class-opt-idle-warn-close = Warn, then close
class-opt-idle-close = Close immediately
class-opt-none = (not set)
class-action-activate = Activate
class-action-deactivate = Deactivate
class-action-permissions = Program Permissions
class-action-update-menus = Update All User Menus
class-dialog-new = New Permission Class
class-dialog-edit = Edit Permission Class { $code }
class-dialog-update-menus = Update All User Menus
class-confirm-update-menus = Set every user's menu to the default menu of their permission class (classes without a default menu are skipped). Continue?
class-hint-idle = Idle action and idle time apply only when idle control is "By permission class".
class-msg-saved = Permission class { $code } saved
class-msg-deleted = Permission class { $code } deleted
class-msg-activated = Permission class { $code } activated
class-msg-deactivated = Permission class { $code } deactivated
class-msg-confirmed = Permission class { $code } confirmed
class-msg-unconfirmed = Permission class { $code } unconfirmed
class-msg-copied = Permission class { $from } copied to { $to }
class-field-copy-as = Copy as
class-msg-menus-updated = Updated the menu of { $count } users
class-note-permissions = Program run permissions (p_zy) and print permissions (p_zy2) will be ported later; a new permission class starts without program permissions.
# Query mode hints (WQ2: code or Traditional Chinese label)
class-hint-qbe-idle-control = 1 Not controlled | 2 By class | 3 By system
class-hint-qbe-idle-action = 1 Warn only | 2 Warn, then close | 3 Close immediately
