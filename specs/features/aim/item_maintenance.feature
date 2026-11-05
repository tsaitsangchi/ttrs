# language: zh-TW
# TipTop 對應：aimi100 料件基本資料建立 / ima_file（以客製版為準：topcust/cim/4gl/aimi100.4gl、topcust/cim/4fd/aimi100.4fd）
# 新系統名稱：Item Maintenance 料件基本資料維護（item-maintenance，ADR 0008 已登錄）
# 規則來源：topcust/cim/4gl/aimi100.4gl（8,385 行；標準版 tiptop/aim/4gl/aimi100.4gl 8,075 行，差異約 470 行）、
#           topcust/cim/4fd/aimi100.4fd、topcust/config/4ad/0/cim/aimi100.4ad（49 個功能）、tiptop/aim/sch/ima_file.sch（264 欄）。
# 狀態：@draft——範圍 IM1–IM10 依建議確認（Key User，2026-11-05）；IM3、IM4、IM8 待 TOPTEST 資料統計結果（先跳過）。
#       第一階段規則已萃取（見下）；IM11–IM20 依建議確認（Key User，2026-11-05；IM15 未確認正式區有開啟，第一階段不做自動編碼）。
#       依 IM16 先移植分類碼、產品分類、單位、料件分群，再寫本規格情境。
#       接著逐欄萃取第一階段規則、寫情境，並取代早期骨架 item_master.feature。
#
# 萃取摘要：
#   畫面（客製 4fd）：表頭 19 欄＋8 個分頁＋清單頁。
#     表頭：ima01 料件編號、ima02 品名、ima021 規格、ima06 分群碼、ima08 來源碼、ima13 規格組件料件編號、ima05 目前使用版本、
#           ima140 Phase Out 否、ima03 目錄參考號碼、ima1010 狀態碼、ima93 新料件處理狀況、ima915 供應商管制、ima916 資料來源、
#           ta_ima01 訂單無效否（客製）、ima131 產品分類編號、ima04 工程圖號、imgmksg、ta_ima08 申請部門（客製）＋gem02 部門名稱。
#     分頁：資料處理（imauser 等 5 欄）、ICD 資訊（imaicd 12 欄）、工程／計劃／庫存（29 欄，含客製 ta_ima02 PCN）、
#           批序號（ima918–ima925 8 欄）、會計／狀態／分群（10 欄）、料件狀態（ima1001 等 7 欄）、參考日期（7 欄）、
#           自訂欄位（imaud01–imaud15）。
#   功能（49）：新增、查詢、修改、取消（刪除）、作廢、複製、確認／取消確認、訂單無效（客製 order_invalid）、更新多語系名稱、
#     以及連到其他作業：庫存資料（aimi101）、採購資料（aimi103）、生管資料（aimi104）、成本（aimi105–107）、品名規格額外說明（aimi108）、
#     單位換算、分群碼（aimi110）、其他分群碼一～四、資料拋轉（carry）、APS、多屬性子料件、資料下載、批次修改人員等。
#   客製（topcust 與標準版差異）：ta_ima01 訂單無效否（切換動作，紅字顯示）、ta_ima02 PCN、ta_ima08 申請部門（檢查 gem_file）、
#     產品分類碼開窗且不可空白（No:10108018）。ta_ima03 成本中心、ta_ima04–07 採購分類、ta_ima09 安庫否不在 aimi100 畫面。
#   ima_file 264 欄：aimi100 畫面 97 欄；其他作業維護 70 欄（aimi103 採購 30、aimi104 生管 24、aimi101 庫存 8、aimi105 成本 6、其他 2）；
#     不在任何料件畫面 97 欄（程式自動寫入、拋轉用或已不使用）。
#   引用的主檔（欄位檢查需要）：imz_file 料件分群（ima06，aimi110）、其他分群碼（ima09–ima12）、gfe_file 單位（ima25 等）、
#     gem_file 部門（ta_ima08，已移植）、gen_file 員工（已移植）、aag_file 會計科目（ima39 等）、imd／ime 倉庫儲位（ima35、ima36）、
#     產品分類（ima131）。
#
# 確認事項（皆採建議）：
#  IM1. 分階段移植。建議第一階段：
#       | 範圍 | 內容 |
#       | 第一階段（建議） | 表頭 19 欄；分頁「資料處理」「工程／計劃／庫存」「會計／狀態／分群」「料件狀態」「參考日期」；功能：新增、查詢、修改、取消、作廢、複製、確認／取消確認、訂單無效、多語言品名規格 |
#       | 第二階段 | 批序號分頁；庫存資料（aimi101）；料件報表 aimr100 |
#       | 之後各自成案 | 採購資料（aimi103）、生管資料（aimi104）、成本（aimi105–107）、品名規格額外說明（aimi108）、資料拋轉、APS、多屬性子料件 |
#  IM2. 引用的主檔還沒移植（料件分群 imz_file、其他分群碼、產品分類、會計科目、倉庫儲位）。建議：分群碼（aimi110）、其他分群碼、
#       產品分類與料件一起移植（新增料件時會帶入分群碼的預設值，規則上分不開）；會計科目、倉庫儲位先存代碼不檢查、標 verify，
#       等會計與倉庫作業移植時再加檢查。可以嗎？
#  IM3. ICD 資訊分頁（imaicd01–13，IC 設計行業別 aimi100_icd）：公司有使用嗎？未使用則不移植。
#  IM4. 自訂欄位 imaud01–imaud15：有使用嗎？建議用 TOPTEST 實際資料統計各欄非空白筆數再決定（scripts/tiptop/tiptop.py，唯讀）。
#       同樣方法可確認 97 個不在畫面上的欄位哪些有資料。可以由我查 TOPTEST 嗎？
#  IM5. 狀態：TipTop 有 imaacti 資料有效碼、ima1010 狀態碼（開立／確認…）、作廢；TTRS 早期骨架用 unconfirmed／confirmed／voided。
#       改照 TipTop 重新萃取（ima1010＋imaacti 兩個欄位），取代骨架的狀態模型。可以嗎？
#  IM6. 料件編號長度：ima01 varchar2(40)，TTRS 骨架的限制不同；改照 TipTop 40 字，並重新萃取編號規則（自動編碼 No:A086）。
#  IM7. 多語言品名、規格（「更新多語系名稱資料」、TQC-6C0024）：依 ADR 0005 建 item_text 表（料號＋語系），畫面用 MultilingualField。
#  IM8. 客製欄位：ta_ima01 訂單無效否、ta_ima02 PCN、ta_ima08 申請部門照客製移植；不在畫面上的 ta_ima03–07、ta_ima09
#       是否有其他程式使用？（建議同 IM4 以資料統計確認，未使用則不移植）
#  IM9. 營運中心：料件是工廠層資料（ADR 0007），每個營運中心各自一份，沿用。資料拋轉（carry，把料件複製到其他營運中心）延後（IM1）。
#  IM10. 既有資料：TTRS 目前料件只有 5 欄（料號、品名、規格、庫存單位、狀態）；第一階段以新的 migration 擴充，
#        早期骨架的 item_master.feature 由本規格取代、aimr100 報表（item_report.feature）等第一階段完成後再做。

#
# ===== 第一階段萃取（topcust/cim/4gl/aimi100.4gl 行號；IM3、IM4、IM8 的 TOPTEST 統計先跳過）=====
#
# 狀態（imaacti 資料有效碼＋ima1010 狀態碼；aimi100_sub.4gl 為確認段）：
#   - 新增：imaacti＝P（處理中）、ima1010＝0（開立）（i100_default :3018、:3043）。
#   - 確認（i100sub_y_chk／y_upd）：已確認（imaacti＝Y）→ 9023；無效（N）→ aim-153；他廠拋轉來的料件 → aoo-045；
#     成功後 ima1010＝1、imaacti＝Y。
#   - 取消確認（i100_notconfirm :4771）：ima1010≠1 或已無效 → atm-365；有多屬性子料件 → aim-451；成功後回到 P／0。
#   - 作廢（i100_x :4042，切換）：開立的 P↔N、已確認的 Y↔N。擋下：有 BOM 或被 BOM 引用（aim-022）、有雜收發單（aim-026）、
#     庫存數量 ima26／ima261／ima262＞0（mfg9165–9167）、有庫存資料 img（mfg9163）；有未結工單／採購單／訂單時只提醒（aim-141）。
#   - 取消（刪除，i100_r :4274）：只限處理中；無效 → aim-153、已確認 → 9023；連同多語言品名規格、單位換算等一併刪除。
# 新增與欄位檢查（i100_i :1369-2332、i100_default :2817-3224）：
#   - 料件編號 ima01：不可重複（-239）；不可含 * : | ? ! % & ^ < >（aim-122）；MISC 開頭須先有 MISC 料件（aim-806），
#     且來源碼一律 Z（aim-805）（i100_chk_ima01 :6156）。
#   - 分群碼 ima06：須存在且有效於料件分群 imz_file（mfg3179、9028）；變更時詢問（mfg5033）後由分群帶入約 100 欄預設值，
#     含客製 ta_ima01、ta_ima02、ta_ima08（i100_set_rel_ima06 :3842）。修改時料件已被使用則只提醒（mfg9199）。
#   - 來源碼 ima08：C T D A M P X K U V R Z S（mfg1001）；非 T 清空規格主件 ima13；非 M、T 時 ima903、ima905＝N（:6342）。
#   - ABC 碼 ima07：A B C（mfg0009）；補貨策略碼 ima37：0–6（mfg1003），0、5 只限來源碼 M S P V Z（mfg3201）。
#   - 經濟訂購量 ima51、平均訂購量 ima52：須＞0（mfg1322）；安全存量等 ima27、ima271、ima28、ima71：不可＜0（mfg4012）。
#   - 是／否欄位（ima14、ima15、ima24、ima70、ima105、ima107、ima147、ima903、ima911）：Y／N（mfg1002）。
#   - 其他分群碼一～四 ima09–ima12：分類碼 azf_file 類別 D／E／F／G（mfg1306）；材料類別 ima109：azf 類別 8。
#   - 產品分類 ima131：須存在於 oba_file，客製為必填（aim-372，No:10108018）。
#   - 庫存單位 ima25：必填（asf-031）、須存在於 gfe_file（mfg1200）；變更時詢問（aim-020）。
#   - 預設倉庫 ima35（imd_file，mfg1100）、儲位 ima36（ime_file，mfg1101）；倉管員 ima23（gen_file，aoo-001）；
#     存貨科目 ima39、ima391（aag_file 非統制帳戶，anm-001）。
#   - 雙單位 ima906–ima908：依系統參數 sma115、sma122（aim-998、asm-322、asm-323）。
#   - 預設值：ABC 碼 A、來源碼 P、低階碼 99、經濟訂購量 1、平均訂購量 1、最近盤點日＝今天、建檔日＝今天、
#     ima93＝NNNNNNNN、ima916 資料來源＝目前營運中心、訂單無效否 ta_ima01＝Y（客製）。
# 客製功能（menu :1214-1233）：
#   - 訂單無效（i100_order_invalid :8003）：切換 ta_ima01，記錄修改者與日期（有查庫存但未拿來擋）。
#   - 安全庫存（i100_safe_stock :8056）：只修改 ima27、ima271、ima28、ima71 的快速畫面。
#   - IN_PK 規格說明（i100_in_pk_spec :8135）：客製表 tc_imr_file（種類、片種、倍數、容量…12 欄，開窗 q_tc_oat）。
# 一起移植的主檔（IM2）：料件分群 imz_file（aimi110，2,885 行，欄位與料件相仿）、分類碼 azf_file（aooi300；客製 topcust/coo/aooi080）、
#   產品分類 oba_file（axmi110）。單位 gfe_file（aooi101）TTRS 只有骨架。
#
# 第一階段確認事項（IM11–IM20，皆採建議）：
#  IM11. 狀態照 TipTop 兩個欄位：資料有效碼（處理中／已確認／無效）＋狀態碼（開立／確認），轉換規則照上列。
#        畫面顯示一個狀態（處理中、已確認、無效）即可，可以嗎？
#  IM12. 作廢的檢查引用未移植的模組（BOM、雜收發、庫存、工單、採購、訂單）：第一階段先檢查 TTRS 已有的庫存異動，
#        其他等各模組移植時加入（列在 legacy-map 待辦），可以嗎？
#  IM13. 他廠拋轉相關（ima916 資料來源、aoo-044／aoo-045、s_field_chk 欄位管控 aoo-043、確認後自動拋轉）：資料拋轉延後（IM1），
#        第一階段 ima916 記錄目前營運中心、不做這些檢查，可以嗎？
#  IM14. 雙單位 ima906–ima908 依系統參數 sma115（未移植）：第一階段固定單一單位（ima906＝1），欄位保留，可以嗎？
#  IM15. 自動編碼（aza28＋s_auno，No:A086）：正式區有開啟嗎？未開啟則第一階段不做。
#  IM16. 依序移植：先分類碼（aooi300／客製 aooi080）、產品分類（axmi110）、單位（aooi101 補齊），再料件分群（aimi110），最後料件（aimi100）。
#        每支都照萃取流程各寫規格，可以嗎？
#  IM17. 客製「安全庫存」快速畫面與「IN_PK 規格說明」（tc_imr_file）：放第二階段，可以嗎？
#  IM18. 客製預設「訂單無效否＝Y」（新料件預設不可下訂單）照移植，可以嗎？
#  IM19. 多屬性子料件（imaag、ima151、@CHILD、aim-451）與 MES、PLM 整合：不移植（行業別功能、未整合），可以嗎？
#  IM20. MISC 料件規則（MISC 開頭須先有 MISC、來源碼一律 Z）照移植，可以嗎？

@draft
功能: 料件基本資料維護（Item Maintenance）
  身為 物管人員
  我想要 照 TipTop aimi100 維護料件基本資料
  以便 採購、銷售、生產與庫存作業引用同一份料件
