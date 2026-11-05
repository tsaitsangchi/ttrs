# 系统寄出的信件（ADR 0010 R15）；三个语系的 ID 必须一致
mail-password-reset-subject = TTRS 重设密码
mail-password-reset-body =
    { $name } 您好：

    我们收到用户代号 { $user } 重设 TTRS 密码的申请。
    请在 30 分钟内打开下列链接设置新密码（链接只能使用一次）：

    { $link }

    如果不是您本人申请，请忽略这封信，原来的密码不会变更。
mail-password-changed-subject = TTRS 密码已变更
mail-password-changed-body =
    { $name } 您好：

    用户代号 { $user } 的 TTRS 密码已经通过重设链接变更，原来已登录的连接都已结束。

    如果不是您本人操作，请立即联系系统管理员。
