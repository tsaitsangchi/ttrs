# 系統寄出的信件（ADR 0010 R15）；三個語系的 ID 必須一致
mail-password-reset-subject = TTRS 重設密碼
mail-password-reset-body =
    { $name } 您好：

    我們收到使用者代號 { $user } 重設 TTRS 密碼的申請。
    請在 30 分鐘內開啟下列連結設定新密碼（連結只能使用一次）：

    { $link }

    如果不是您本人申請，請忽略這封信，原本的密碼不會變更。
mail-password-changed-subject = TTRS 密碼已變更
mail-password-changed-body =
    { $name } 您好：

    使用者代號 { $user } 的 TTRS 密碼已經透過重設連結變更，原本已登入的連線都已結束。

    如果不是您本人操作，請立即聯絡系統管理員。
