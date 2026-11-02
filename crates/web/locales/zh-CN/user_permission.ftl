# User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw）
# 語系：zh-CN。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

uperm-breadcrumb = 系统管理 / User Special Permission Maintenance
uperm-title = 用户特殊权限维护
uperm-list-title = 特殊权限
uperm-note = 用户有特殊权限时，只使用这里列出的权限类别与程序，不再使用用户维护设置的权限类别；全部删除后回到原权限类别。多行给同一程序时取较宽的权限（随服务器端权限检查上线）。
uperm-kind-class = 权限类别
uperm-kind-program = 程序
uperm-field-user = 用户代号
uperm-field-user-name = 用户名称
uperm-field-user-class = 用户维护的权限类别
uperm-field-state = 目前使用
uperm-state-class = 用户维护的权限类别
uperm-state-special = 特殊权限
uperm-field-line = 项次
uperm-field-kind = 分类
uperm-field-target = 权限类别／程序代号
uperm-field-target-name = 说明／程序名称
uperm-field-include-own = 先加入原权限类别 { $class }
uperm-field-target-user = 复制到用户
uperm-msg-none = 没有特殊权限，使用用户维护设置的权限类别
uperm-msg-removed = 已移除第 { $line } 项特殊权限
uperm-msg-added = 已新增 { $target } 的特殊权限
uperm-msg-copied = 已复制 { $count } 笔特殊权限到 { $target }
uperm-dialog-add = 新增特殊权限（{ $user }）
uperm-dialog-edit = 修改特殊权限（{ $user } / { $program }）
uperm-dialog-copy = 复制 { $user } 的特殊权限
uperm-dialog-delete-all = 删除 { $user } 的全部特殊权限
uperm-hint-first = 这是此用户的第一笔特殊权限；有特殊权限后就不再使用用户维护的权限类别，建议先加入原权限类别。
uperm-hint-add = 权限类别须有效；程序不可为菜单，允许执行功能默认为程序的全部功能、数据权限 0，新增后可再修改。
uperm-hint-copy = 目标用户必须还没有任何特殊权限。
uperm-confirm-delete-all = 将删除此用户的全部特殊权限，之后改用用户维护设置的权限类别。是否执行？
