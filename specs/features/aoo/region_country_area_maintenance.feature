# language: zh-TW
# TipTop 對應：aooi100 區域別資料維護 / gea_file（區域別檔）
#              aooi090 國別資料維護 / geb_file（國別檔，geb03 隸屬區域 → gea01）
#              aooi110 地區資料維護 / geo_file（地區資料檔，geo03 隸屬國別 → geb01）
# 新系統名稱：Region Maintenance 區域別維護（region-maintenance）、Country Maintenance 國別維護（country-maintenance）、
#             Area Maintenance 地區維護（area-maintenance）（RC1）
# 規則來源：tiptop/aoo/4gl/aooi100.4gl:216-230（區域重複 -239）、:231-238（資料有效 Y／N）、:239（刪除）、:353（查詢欄位 gea01、gea02、geaacti）；
#           tiptop/aoo/4gl/aooi090.4gl:220（資料有效預設 Y）、:232-234（新增：建立者、日期）、:245-259（國別重複 -239）、
#           :260-267（隸屬區域須存在 aoo-061、須有效 9028，開窗 q_gea）、:268-275（資料有效）、:276（刪除）、
#           :419（查詢欄位 geb01、geb02、geb03、gebacti）；
#           tiptop/aoo/4gl/aooi110.4gl:224-238（地區重複 -239）、:239-246（隸屬國別須存在 aoo-061、須有效 9028，開窗 q_geb）、
#           :247-254（資料有效）、:255（刪除）、:399（查詢欄位 geo01、geo02、geo03、geoacti）；
#           tiptop/aoo/sch/gea_file.sch、geb_file.sch、geo_file.sch。無 topcust 覆寫。
# 狀態：RC1–RC7 依建議確認（Key User，2026-11-10），進入 Red
#
# TipTop 的做法（萃取）：
#   - 三支都是多筆單身表格，代號 10 字、名稱 80 字、資料有效預設 Y。
#   - 階層：區域（gea，如亞洲）→ 國別（geb，geb03 隸屬區域）→ 地區（geo，geo03 隸屬國別）。隸屬欄位可空白。
#   - 輸入隸屬區域／國別時檢查存在且有效；刪除時不檢查是否被下層或客戶、供應商引用。
#   - 功能只有 query、detail、output、exporttoexcel；其餘標準功能鈕依 ADR 0013 補上。
#
# 確認事項（皆採建議）：
#  RC1. 三支作業各自一個畫面（照 TipTop 作業代號），版面比照各自的 4fd 單一表格。可以嗎？
#  RC2. 範圍：全公司共用的代碼，放共用 schema（scope: system），同單位（UN2）。可以嗎？
#  RC3. 隸屬區域、隸屬國別照 TipTop 可空白；有填時須存在（REGION_NOT_FOUND、COUNTRY_NOT_FOUND）且有效（REGION_INACTIVE、
#       COUNTRY_INACTIVE）。可以嗎？
#  RC4. 刪除：區域已被國別引用、國別已被地區引用時不可刪除，請改為停用（REGION_IN_USE、COUNTRY_IN_USE）；
#       TipTop 不檢查。客戶、供應商（occ、pmc）移植後再加入它們的引用檢查。可以嗎？
#  RC5. 代號建立後不可修改、區分大小寫、最多 10 字（同 UN3）。可以嗎？
#  RC6. 名稱不做多語言（TipTop 無語言別欄位）。可以嗎？
#  RC7. 不移植的欄位：
#        | 欄位 | 說明 | 原因 |
#        | geauser、geagrup、geamodu、geadate | 所有者、部門、修改者、修改日 | 改用 TTRS 標準欄位（ADR 0012） |
#        | gebuser、gebgrup、gebmodu、gebdate | 同上 | 同上 |
#        | geouser、geogrup、geomodu、geodate | 同上 | 同上 |
#
# 標準功能鈕（ADR 0013）：三支皆有新增、修改、刪除、查詢、列印、匯出 Excel、上下筆、停用／啟用、複製、異動記錄、
#   確認／取消確認（用 TTRS 標準確認欄位 SA9-1，已確認不可刪除）。
#   列印與匯出欄位：代號、名稱、隸屬區域／國別（含名稱）、資料有效。

功能: 區域別、國別、地區維護（Region, Country and Area Maintenance）
  身為 業務與採購人員
  我想要 維護區域、國別與地區的階層
  以便 客戶、供應商與統計報表依地理分類

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  # aooi100.4gl 新增，資料有效預設 Y
  場景: RC-1 新增區域
    當 使用者新增區域 "AS"，名稱 "亞洲"
    那麼 操作應成功
    而且 區域 "AS" 應為有效

  # aooi100.4gl:216-230 重複
  場景: RC-2 區域不可重複
    假設 已有區域 "AS"
    當 使用者新增區域 "AS"，名稱 "亞太"
    那麼 應出現錯誤 "REGION_ALREADY_EXISTS"

  # aooi090.4gl:232-234 新增國別
  場景: RC-3 新增國別並指定隸屬區域
    假設 已有區域 "AS"
    當 使用者新增國別 "TW"，名稱 "台灣"，隸屬區域 "AS"
    那麼 操作應成功
    而且 國別 "TW" 的隸屬區域應為 "AS"

  # RC3 隸屬區域可空白
  場景: RC-4 國別的隸屬區域可空白
    當 使用者新增國別 "JP"，名稱 "日本"
    那麼 操作應成功

  # aooi090.4gl:260-267 隸屬區域須存在、須有效
  場景: RC-5 隸屬區域須存在
    當 使用者新增國別 "TW"，名稱 "台灣"，隸屬區域 "XX"
    那麼 應出現錯誤 "REGION_NOT_FOUND"

  場景: RC-6 隸屬區域須有效
    假設 已有區域 "AS"
    而且 區域 "AS" 已停用
    當 使用者新增國別 "TW"，名稱 "台灣"，隸屬區域 "AS"
    那麼 應出現錯誤 "REGION_INACTIVE"

  # aooi090.4gl:245-259 重複
  場景: RC-7 國別不可重複
    假設 已有國別 "TW"
    當 使用者新增國別 "TW"，名稱 "中華民國"
    那麼 應出現錯誤 "COUNTRY_ALREADY_EXISTS"

  # aooi110.4gl:239-246 隸屬國別須存在、須有效
  場景: RC-8 新增地區並指定隸屬國別
    假設 已有國別 "TW"
    當 使用者新增地區 "TPE"，名稱 "台北"，隸屬國別 "TW"
    那麼 操作應成功
    而且 地區 "TPE" 的隸屬國別應為 "TW"

  場景: RC-9 隸屬國別須存在
    當 使用者新增地區 "TPE"，名稱 "台北"，隸屬國別 "XX"
    那麼 應出現錯誤 "COUNTRY_NOT_FOUND"

  場景: RC-10 隸屬國別須有效
    假設 已有國別 "TW"
    而且 國別 "TW" 已停用
    當 使用者新增地區 "TPE"，名稱 "台北"，隸屬國別 "TW"
    那麼 應出現錯誤 "COUNTRY_INACTIVE"

  # aooi110.4gl:224-238 重複
  場景: RC-11 地區不可重複
    假設 已有地區 "TPE"
    當 使用者新增地區 "TPE"，名稱 "台北市"
    那麼 應出現錯誤 "AREA_ALREADY_EXISTS"

  # RC4 已被引用不可刪除
  場景: RC-12 已被國別引用的區域不可刪除
    假設 已有區域 "AS"
    而且 已有國別 "TW"，隸屬區域 "AS"
    當 使用者刪除區域 "AS"
    那麼 應出現錯誤 "REGION_IN_USE"

  場景: RC-13 已被地區引用的國別不可刪除
    假設 已有國別 "TW"
    而且 已有地區 "TPE"，隸屬國別 "TW"
    當 使用者刪除國別 "TW"
    那麼 應出現錯誤 "COUNTRY_IN_USE"

  場景: RC-14 刪除未被引用的地區
    假設 已有地區 "TPE"
    當 使用者刪除地區 "TPE"
    那麼 操作應成功
    而且 地區 "TPE" 應不存在

  # 停用、啟用
  場景: RC-15 停用與啟用國別
    假設 已有國別 "TW"
    當 使用者停用國別 "TW"
    那麼 國別 "TW" 的資料有效應為 "否"
    當 使用者啟用國別 "TW"
    那麼 國別 "TW" 的資料有效應為 "是"

  # 修改隸屬區域（ADR 0012 版本比對）
  場景: RC-16 修改國別的隸屬區域
    假設 已有區域 "AS"
    而且 已有區域 "EA"
    而且 已有國別 "TW"，隸屬區域 "AS"
    當 使用者將國別 "TW" 的隸屬區域改為 "EA"
    那麼 國別 "TW" 的隸屬區域應為 "EA"

  # aooi090.4gl:419 查詢
  場景: RC-17 依隸屬區域查詢國別
    假設 已有區域 "AS"
    而且 已有區域 "EU"
    而且 已有國別 "TW"，隸屬區域 "AS"
    而且 已有國別 "JP"，隸屬區域 "AS"
    而且 已有國別 "DE"，隸屬區域 "EU"
    當 使用者以下列條件查詢國別:
      | 欄位     | 條件 |
      | 隸屬區域 | AS   |
    那麼 查詢結果應為 "JP,TW" 共 2 筆

  # ADR 0013 複製、確認
  場景: RC-18 複製地區
    假設 已有國別 "TW"
    而且 已有地區 "TPE"，隸屬國別 "TW"
    當 使用者將地區 "TPE" 複製為 "NTP"
    那麼 操作應成功
    而且 地區 "NTP" 的隸屬國別應為 "TW"
    而且 地區 "NTP" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: RC-19 已確認的區域不可刪除
    假設 已有區域 "AS"
    而且 區域 "AS" 已確認
    當 使用者刪除區域 "AS"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
