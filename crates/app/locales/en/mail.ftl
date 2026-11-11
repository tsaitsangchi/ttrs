# Mail sent by the system (ADR 0010 R15); IDs must match across locales
mail-password-reset-subject = Reset your TTRS password
mail-password-reset-body =
    Hello { $name },

    We received a request to reset the TTRS password for user ID { $user }.
    Open the link below within 30 minutes to set a new password (the link works only once):

    { $link }

    If you did not request this, ignore this email; your password will not change.
mail-password-changed-subject = Your TTRS password was changed
mail-password-changed-body =
    Hello { $name },

    The TTRS password for user ID { $user } was changed with a reset link, and all signed-in sessions were ended.

    If you did not do this, contact your system administrator immediately.
mail-rate-job-failed-subject = TTRS daily exchange rate job failed ({ $date }) 每日匯率排程
mail-rate-job-failed-body =
    Some steps of the daily exchange rate job on { $date } failed:

    { $detail }

    Check today's rates in Daily Exchange Rates; if the Bank of Taiwan site cannot be downloaded, upload its CSV on that page.
