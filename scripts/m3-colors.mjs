// 產生 Material 3 色彩 token → crates/web/assets/material/m3-colors.css
//
// 用法（套件裝在暫存目錄，不進專案）：
//   mkdir -p /tmp/m3 && (cd /tmp/m3 && npm install --no-save @material/material-color-utilities@0.4.0)
//   node --experimental-specifier-resolution=node scripts/m3-colors.mjs /tmp/m3 "#1E5AA8" \
//     | (echo "/* 自動產生，請勿手改。重新產生：scripts/m3-colors.mjs（說明見檔頭） */"; cat) \
//     > crates/web/assets/material/m3-colors.css
//
// 換品牌色：改第二個參數（種子色）。方案採 M3 預設的 SchemeTonalSpot。
// 0.4.0 的 import 沒有副檔名，Node 18 需要 --experimental-specifier-resolution=node。
const lib = await import(process.argv[2] + '/node_modules/@material/material-color-utilities/index.js');
const { Hct, SchemeTonalSpot, MaterialDynamicColors, argbFromHex, hexFromArgb, customColor } = lib;
const seed = process.argv[3];
const roles = ['primary','onPrimary','primaryContainer','onPrimaryContainer','secondary','onSecondary',
 'secondaryContainer','onSecondaryContainer','tertiary','onTertiary','tertiaryContainer','onTertiaryContainer',
 'error','onError','errorContainer','onErrorContainer','surface','onSurface','surfaceVariant','onSurfaceVariant',
 'surfaceContainerLowest','surfaceContainerLow','surfaceContainer','surfaceContainerHigh','surfaceContainerHighest',
 'surfaceDim','surfaceBright','outline','outlineVariant','inverseSurface','inverseOnSurface','inversePrimary',
 'scrim','shadow'];
const mdc = typeof MaterialDynamicColors.primary === 'object' ? MaterialDynamicColors : new MaterialDynamicColors();
const kebab = s => s.replace(/[A-Z]/g, m => '-' + m.toLowerCase());
function scheme(dark) {
  const s = new SchemeTonalSpot(Hct.fromInt(argbFromHex(seed)), dark, 0);
  return roles.map(r => `  --md-sys-color-${kebab(r)}: ${hexFromArgb(mdc[r].getArgb(s))};`).join('\n');
}
// ERP 狀態色：M3 只有 error，另加 success / warning 並與種子色協調
function custom(dark) {
  return [['success','#2E7D32'],['warning','#B26A00']].map(([name,value]) => {
    const c = customColor(argbFromHex(seed), { name, value: argbFromHex(value), blend: true })[dark ? 'dark' : 'light'];
    return [`  --md-sys-color-${name}: ${hexFromArgb(c.color)};`, `  --md-sys-color-on-${name}: ${hexFromArgb(c.onColor)};`,
      `  --md-sys-color-${name}-container: ${hexFromArgb(c.colorContainer)};`, `  --md-sys-color-on-${name}-container: ${hexFromArgb(c.onColorContainer)};`].join('\n');
  }).join('\n');
}
console.log(`/* 由 material-color-utilities 0.4.0 產生：SchemeTonalSpot，種子色 ${seed}；success/warning 為與種子色協調（harmonize）的自訂色 */`);
console.log(`.md-theme-light {\n  color-scheme: light;\n${scheme(false)}\n${custom(false)}\n}\n`);
console.log(`.md-theme-dark {\n  color-scheme: dark;\n${scheme(true)}\n${custom(true)}\n}`);
