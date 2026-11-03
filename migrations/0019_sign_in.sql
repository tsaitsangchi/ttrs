-- 登入與切換營運中心（TipTop: weblogin、aoos901；specs/features/azz/sign_in.feature）
-- zxy03＝ALL（可使用全部營運中心，s_chkdbs）：user_plant.plant_id 參照 plant，無法存 ALL，改以旗標記錄（SQ5）
ALTER TABLE user_account ADD COLUMN all_plants boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN user_account.all_plants IS 'zxy03＝ALL：可使用全部營運中心（TipTop zxy_file）';
