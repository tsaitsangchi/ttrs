# User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw）
# 語系：en。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

uperm-breadcrumb = System Admin / User Special Permission Maintenance
uperm-title = User Special Permission Maintenance
uperm-list-title = Special Permissions
uperm-note = A user with special permissions uses only the classes and programs listed here, not the class set in User Maintenance; deleting all rows returns the user to that class. When several rows give the same program, the wider permission applies (with the server-side permission check).
uperm-kind-class = Permission Class
uperm-kind-program = Program
uperm-field-user = User Code
uperm-field-user-name = User Name
uperm-field-user-class = Class in User Maintenance
uperm-field-state = In Effect
uperm-state-class = Class in User Maintenance
uperm-state-special = Special permissions
uperm-field-line = Line
uperm-field-kind = Kind
uperm-field-target = Class / Program
uperm-field-target-name = Description / Program Name
uperm-field-include-own = Add the user's own class { $class } first
uperm-field-target-user = Copy to User
uperm-msg-none = No special permissions; the class from User Maintenance applies
uperm-msg-removed = Removed special permission line { $line }
uperm-msg-added = Added special permission for { $target }
uperm-msg-copied = Copied { $count } special permissions to { $target }
uperm-dialog-add = Add Special Permission ({ $user })
uperm-dialog-edit = Edit Special Permission ({ $user } / { $program })
uperm-dialog-copy = Copy Special Permissions of { $user }
uperm-dialog-delete-all = Delete All Special Permissions of { $user }
uperm-hint-first = This is the user's first special permission; from now on the class in User Maintenance no longer applies, so adding it first is recommended.
uperm-hint-add = The class must be active; a program cannot be a menu. Allowed actions start with all of the program's actions and data permissions with 0; you can edit them afterwards.
uperm-hint-copy = The target user must have no special permissions yet.
uperm-confirm-delete-all = Delete all special permissions of this user? The class from User Maintenance will apply again.
