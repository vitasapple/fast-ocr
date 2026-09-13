#!/usr/bin/env node

/**
 * 大梨 OCR - 商业授权卡密批量生成工具 (支持交互式选择)
 * 
 * 功能：
 *   - 交互式询问：生成【永久买断码】还是【一年尝鲜码】
 *   - 交互式输入生成数量与导出文件名
 *   - 亦支持命令行参数直接运行
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const readline = require('readline');

// 必须与 src/index.html 中的 DALI_SALT 保持严格一致
const DALI_SALT = "DALI_PRO_ENTERPRISE_SECRET_SALT_2026";
// 排除容易混淆的 0, O, 1, I
const CHAR_SET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

function getRandomChars(len) {
  let res = "";
  for (let i = 0; i < len; i++) {
    const idx = Math.floor(Math.random() * CHAR_SET.length);
    res += CHAR_SET[idx];
  }
  return res;
}

// 种子规则：
// 永久码: 种子以 'L' 开头 (如 L8K2)
// 一年尝鲜码: 种子以 'Y' 开头 (如 Y9F3)
function getSeedForType(type) {
  const prefix = type === "annual" ? "Y" : "L";
  return prefix + getRandomChars(3);
}

function generateSingleKey(seed) {
  const hash = crypto
    .createHash('sha256')
    .update(`DALI-${seed}::${DALI_SALT}`)
    .digest('hex')
    .toUpperCase();
  const c1 = hash.slice(0, 4);
  const c2 = hash.slice(4, 8);
  return `DALI-${seed}-${c1}-${c2}`;
}

function verifyKey(key) {
  const match = key.match(/^DALI-([A-Z0-9]{4})-([A-F0-9]{4})-([A-F0-9]{4})$/);
  if (!match) return false;
  const seed = match[1];
  const checksum = match[2] + match[3];
  const hash = crypto
    .createHash('sha256')
    .update(`DALI-${seed}::${DALI_SALT}`)
    .digest('hex')
    .toUpperCase();
  return checksum === hash.slice(0, 8);
}

function doGenerate(type, count, customFilename) {
  const typeName = type === "annual" ? "一年尝鲜码 (365天)" : "永久买断码 (终身版)";
  const defaultFile = type === "annual" ? `一年尝鲜码_${count}个.txt` : `永久买断码_${count}个.txt`;
  const outputFile = customFilename || defaultFile;

  console.log(`\n------------------------------------------------------`);
  console.log(`🚀 开始批量生成:`);
  console.log(`• 授权类型: ${typeName}`);
  console.log(`• 生成数量: ${count} 个`);
  console.log(`• 卡密前缀: ${type === "annual" ? "DALI-Y*** (年度标识)" : "DALI-L*** (终身标识)"}`);

  const generatedKeys = new Set();
  const seedSet = new Set();

  while (generatedKeys.size < count) {
    const seed = getSeedForType(type);
    if (seedSet.has(seed)) continue;
    seedSet.add(seed);

    const key = generateSingleKey(seed);
    if (verifyKey(key)) {
      generatedKeys.add(key);
    }
  }

  const keyList = Array.from(generatedKeys);
  const outputPath = path.isAbsolute(outputFile) ? outputFile : path.join(__dirname, '..', outputFile);

  fs.writeFileSync(outputPath, keyList.join('\n') + '\n', 'utf8');

  console.log(`\n✅ 生成成功！已导出 ${keyList.length} 个完全唯一的有效激活码！`);
  console.log(`📁 文件保存至: ${outputPath}`);
  console.log(`\n前 5 个卡密预览:`);
  keyList.slice(0, 5).forEach((k, i) => {
    console.log(`  [${i + 1}] ${k}`);
  });
  console.log(`...\n`);
  console.log(`💡 使用提示: 打开面包多/爱发电 -> 兑换码/卡密 -> 批量粘贴该文件内容即可！\n`);
}

// 检查是否传入了命令行参数
const args = process.argv.slice(2);
if (args.length > 0) {
  // 支持命令直接运行: node scripts/generate_keys.js [type: 1|2|life|year] [count] [filename]
  let type = "lifetime";
  if (args[0] === "2" || args[0] === "year" || args[0] === "annual") {
    type = "annual";
  }
  const count = parseInt(args[1], 10) || 100;
  const filename = args[2] || null;
  doGenerate(type, count, filename);
  process.exit(0);
}

// 交互式提示
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout
});

console.log(`\n======================================================`);
console.log(`🍐 大梨 OCR 商业激活码生成中心`);
console.log(`======================================================`);
console.log(`请选择要生成的激活码类型:`);
console.log(`  [1] 永久买断码 (PRO 终身授权)`);
console.log(`  [2] 一年尝鲜码 (PRO 365天授权)`);

rl.question(`\n请输入序号 (1 或 2，默认 1): `, (typeAns) => {
  const choice = (typeAns || "1").trim();
  const type = (choice === "2") ? "annual" : "lifetime";
  const typeName = (type === "annual") ? "一年尝鲜码" : "永久买断码";

  rl.question(`\n请输入需要生成的数量 (默认 100): `, (countAns) => {
    const count = parseInt((countAns || "100").trim(), 10) || 100;

    const defaultFilename = `${typeName}_${count}个.txt`;
    rl.question(`\n请输入导出文件名 (直接回车默认: ${defaultFilename}): `, (fileAns) => {
      const filename = (fileAns || "").trim() || defaultFilename;
      rl.close();
      doGenerate(type, count, filename);
    });
  });
});
