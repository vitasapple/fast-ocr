/**
 * 大梨 OCR - 官方聚合云函数服务端 (Cloudflare Worker)
 * 
 * 域名: api.daliocr.cn
 * 
 * 聚合接口与页面:
 *   - GET  /                API 网关状态与接口说明
 *   - GET  /update          版本检查与热更新接口 (客户端语义化版本对比、强制/非强制更新)
 *   - GET  /notice          远程运营公告与紧急通知接口 (活动横幅、突发故障通告、社群通知)
 *   - POST /feedback        用户意见与异常崩溃上报接口 (支持自动推送微信/飞书/企微机器人)
 *   - GET  /admin           开发者专属管理看板 (在线直接查看所有用户意见与报错，带密码保护)
 *   - POST /pay/webhook     面包多/爱发电等支付平台自动发货回调预留钩子
 */

// ============================================================================
// 1. 全局运营与版本配置 (直接在 Cloudflare 控制台修改保存，全球秒级生效)
// ============================================================================
const CONFIG = {
  // 软件版本更新控制
  version: {
    latest: "0.1.0",           // 当前最新版本号
    minSupported: "0.0.1",     // 最低兼容版本，低于此版本的客户端将被判定为 force_update=true (强制更新)
    title: "大梨 OCR v0.1.0 稳定版发布",
    changelog: [
      "1. 升级方案 A 智能表格排版重构，复杂合并单元格对齐率大幅提升",
      "2. 深度适配 Apple Silicon M 系列芯片与 Windows 本地 CPU 向量加速",
      "3. 优化「大梨快传」手机扫码互传性能与局域网重连机制",
      "4. 修复极端情况下全局截屏快捷键取词偶发失效的问题",
      "5. 新增单据与表格一键无损导出 Excel / Word / Markdown"
    ].join("\n"),
    downloadUrl: "https://daliocr.cn/#download",
    pubDate: "2026-09-16",
    // 各平台独立安装包直链配置 (若无需独立直链则回退到 downloadUrl)
    packages: {
      "macos_arm64": {
        name: "DaLi-OCR-0.1.0-arm64.dmg",
        url: "https://daliocr.cn/#download",
        checksum: ""
      },
      "macos_x64": {
        name: "DaLi-OCR-0.1.0-x64.dmg",
        url: "https://daliocr.cn/#download",
        checksum: ""
      },
      "windows_x64": {
        name: "DaLi-OCR-0.1.0-win-x64.exe",
        url: "https://daliocr.cn/#download",
        checksum: ""
      }
    }
  },

  // 运营公告与全局通知
  notice: {
    id: "notice-20260916",     // 公告唯一标识，客户端据此判断是否已读关闭
    enabled: false,            // 开关：设为 true 时客户端会弹出或显示公告
    type: "info",              // 类型: "info" (普通) | "warning" (警告) | "activity" (优惠活动)
    title: "大梨 OCR 官方社群与技术交流通知",
    content: "欢迎加入大梨 OCR 官方交流与售后群，第一时间获取高频表格模板与最新离线模型包！",
    actionLabel: "前往官网查看",
    actionUrl: "https://daliocr.cn/#faq",
    canDismiss: true,          // 是否允许用户关闭或点击“不再提示”
    expireAt: "2026-12-31T23:59:59Z"
  },

  // 开发者后台访问管理密码 (可通过 Cloudflare 环境变量 ADMIN_KEY 覆盖)
  admin: {
    defaultKey: "dali2026"
  }
};

// 内存暂存缓冲区 (即使未绑定 KV，单实例也能暂存最近 50 条反馈)
const MEMORY_FEEDBACK_CACHE = [];

// ============================================================================
// 2. 核心辅助工具函数
// ============================================================================

/**
 * 语义化版本对比 (Semver Compare)
 */
function compareVersions(v1, v2) {
  if (!v1 || !v2) return 0;
  const p1 = v1.replace(/^v/i, "").split(".").map(n => parseInt(n, 10) || 0);
  const p2 = v2.replace(/^v/i, "").split(".").map(n => parseInt(n, 10) || 0);
  const maxLen = Math.max(p1.length, p2.length);

  for (let i = 0; i < maxLen; i++) {
    const num1 = p1[i] || 0;
    const num2 = p2[i] || 0;
    if (num1 > num2) return 1;
    if (num1 < num2) return -1;
  }
  return 0;
}

/**
 * 跨域与 JSON 响应封装
 */
function jsonResponse(data, status = 200, customHeaders = {}) {
  const headers = {
    "Content-Type": "application/json; charset=utf-8",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Requested-With",
    "Cache-Control": "no-store, no-cache, must-revalidate",
    ...customHeaders
  };
  return new Response(JSON.stringify(data, null, 2), { status, headers });
}

function htmlResponse(html, status = 200) {
  return new Response(html, {
    status,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store, no-cache, must-revalidate"
    }
  });
}

// ============================================================================
// 3. 路由分发处理器 (Route Handlers)
// ============================================================================

const handlers = {
  // 网关首页
  async handleRoot(request) {
    return jsonResponse({
      name: "DaLi OCR Cloud API Gateway",
      version: "1.0.0",
      status: "online",
      official_site: "https://daliocr.cn",
      endpoints: {
        "GET /update": "版本检查与客户端升级通知",
        "GET /notice": "远程运营公告与突发维护通知",
        "POST /feedback": "用户问题与日志反馈上报",
        "GET /admin?key=xxx": "开发者在线管理看板 (查看用户反馈)",
        "POST /pay/webhook": "支付平台自动发卡回调预留"
      },
      time: new Date().toISOString()
    });
  },

  // 1. 版本更新检查
  async handleUpdate(request, url) {
    const currentVersion = url.searchParams.get("version") || "0.0.0";
    const platform = (url.searchParams.get("platform") || "macos").toLowerCase();
    const arch = (url.searchParams.get("arch") || "arm64").toLowerCase();
    const channel = url.searchParams.get("channel") || "stable";

    const latestVersion = CONFIG.version.latest;
    const minSupported = CONFIG.version.minSupported;

    // 检查是否有更高版本
    const hasUpdate = compareVersions(latestVersion, currentVersion) > 0;
    // 检查客户端是否低于最低支持版本 (若低于则触发强制更新)
    const isForceUpdate = hasUpdate && compareVersions(currentVersion, minSupported) < 0;

    // 寻找对应平台的安装包配置
    const packageKey = `${platform}_${arch}`;
    const specificPackage = CONFIG.version.packages[packageKey] || null;
    const downloadUrl = (specificPackage && specificPackage.url) ? specificPackage.url : CONFIG.version.downloadUrl;

    return jsonResponse({
      code: 0,
      msg: "success",
      data: {
        has_update: hasUpdate,
        latest_version: latestVersion,
        current_version: currentVersion,
        force_update: isForceUpdate,
        min_supported_version: minSupported,
        title: CONFIG.version.title,
        changelog: CONFIG.version.changelog,
        download_url: downloadUrl,
        package_info: specificPackage,
        pub_date: CONFIG.version.pubDate,
        platform: platform,
        arch: arch,
        channel: channel
      }
    });
  },

  // 2. 远程运营公告
  async handleNotice(request) {
    const notice = CONFIG.notice;
    const now = new Date();
    const isExpired = notice.expireAt && new Date(notice.expireAt) < now;

    if (!notice.enabled || isExpired) {
      return jsonResponse({
        code: 0,
        msg: "no_notice",
        data: {
          has_notice: false
        }
      });
    }

    return jsonResponse({
      code: 0,
      msg: "success",
      data: {
        has_notice: true,
        id: notice.id,
        type: notice.type,
        title: notice.title,
        content: notice.content,
        action_label: notice.actionLabel,
        action_url: notice.actionUrl,
        can_dismiss: notice.canDismiss,
        expire_at: notice.expireAt
      }
    });
  },

  // 3. 用户意见与错误反馈上报
  async handleFeedback(request, url, env) {
    let body = {};
    try {
      body = await request.json();
    } catch (_) {
      return jsonResponse({ code: 4001, msg: "无效的 JSON 数据格式" }, 400);
    }

    const { type, contact, content, version, platform, arch, error_log } = body;
    if (!content || !content.trim()) {
      return jsonResponse({ code: 4002, msg: "反馈内容不能为空" }, 400);
    }

    const feedbackItem = {
      id: `fb_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
      time: new Date().toLocaleString("zh-CN", { timeZone: "Asia/Shanghai" }),
      timestamp: Date.now(),
      type: type || "general",
      contact: (contact || "").trim() || "未留联系方式",
      content: content.trim(),
      client: {
        version: version || "unknown",
        platform: platform || "unknown",
        arch: arch || "unknown"
      },
      error_log: (error_log || "").trim()
    };

    // 存入内存缓存
    MEMORY_FEEDBACK_CACHE.unshift(feedbackItem);
    if (MEMORY_FEEDBACK_CACHE.length > 50) {
      MEMORY_FEEDBACK_CACHE.pop();
    }

    // 持久化到 Cloudflare KV (如果绑定了 DALI_FEEDBACK)
    if (env?.DALI_FEEDBACK) {
      try {
        const kvKey = `fb:${feedbackItem.timestamp}:${feedbackItem.id}`;
        await env.DALI_FEEDBACK.put(kvKey, JSON.stringify(feedbackItem), {
          expirationTtl: 86400 * 90 // 保存 90 天
        });
      } catch (kvErr) {
        console.warn("KV 保存异常:", kvErr);
      }
    }

    // 微信/飞书/企业微信机器人通知 (若配置了 FEEDBACK_WEBHOOK_URL)
    const webhookUrl = env?.FEEDBACK_WEBHOOK_URL;
    if (webhookUrl) {
      try {
        const textPayload = [
          `🔔【大梨 OCR 收到新用户反馈】`,
          `• 类型: ${feedbackItem.type}`,
          `• 联系: ${feedbackItem.contact}`,
          `• 客户端: v${feedbackItem.client.version} (${feedbackItem.client.platform}-${feedbackItem.client.arch})`,
          `• 内容: ${feedbackItem.content}`,
          feedbackItem.error_log ? `• 错误日志: 有 (${feedbackItem.error_log.slice(0, 80)}...)` : null,
          `• 时间: ${feedbackItem.time}`
        ].filter(Boolean).join("\n");

        // 智能兼容 Server酱 / PushPlus / 飞书 / 企微 Webhook 格式
        let postBody = { msg_type: "text", text: textPayload }; // 飞书
        if (webhookUrl.includes("pushplus") || webhookUrl.includes("ftqq")) {
          postBody = { title: "大梨OCR新用户反馈", content: textPayload };
        } else if (webhookUrl.includes("weixin.qq.com")) {
          postBody = { msgtype: "text", text: { content: textPayload } };
        }

        await fetch(webhookUrl, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(postBody)
        });
      } catch (err) {
        console.warn("Webhook 通知发送异常:", err);
      }
    }

    return jsonResponse({
      code: 0,
      msg: "反馈提交成功，非常感谢您的支持与建议！",
      data: { feedback_id: feedbackItem.id }
    });
  },

  // 4. 开发者在线看板页面 (查看用户反馈)
  async handleAdmin(request, url, env) {
    const adminKey = env?.ADMIN_KEY || CONFIG.admin.defaultKey;
    const providedKey = url.searchParams.get("key");

    // 密码验证
    if (providedKey !== adminKey) {
      return htmlResponse(`
        <!DOCTYPE html>
        <html lang="zh-CN">
        <head>
          <meta charset="UTF-8">
          <meta name="viewport" content="width=device-width, initial-scale=1.0">
          <title>大梨 OCR 开发者后台认证</title>
          <style>
            body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #f8fafc; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }
            .card { background: #1e293b; padding: 32px; border-radius: 14px; border: 1px solid #334155; width: 340px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); text-align: center; }
            h2 { margin-top: 0; font-size: 20px; }
            p { color: #94a3b8; font-size: 13px; margin-bottom: 24px; }
            input { width: 100%; box-sizing: border-box; padding: 10px 14px; border-radius: 8px; border: 1px solid #475569; background: #0f172a; color: #fff; font-size: 14px; margin-bottom: 16px; outline: none; }
            button { width: 100%; padding: 10px; border-radius: 8px; border: none; background: #2563eb; color: #fff; font-weight: 600; cursor: pointer; }
            button:hover { background: #1d4ed8; }
          </style>
        </head>
        <body>
          <div class="card">
            <h2>🍐 大梨 OCR 管理看板</h2>
            <p>请输入后台访问密码以查看用户反馈：</p>
            <form method="GET" action="/admin">
              <input type="password" name="key" placeholder="输入管理员访问密钥" required autofocus />
              <button type="submit">进入看板</button>
            </form>
          </div>
        </body>
        </html>
      `, 401);
    }

    // 获取所有反馈记录 (优先从 KV 读，其次读内存)
    let list = [];
    if (env?.DALI_FEEDBACK) {
      try {
        const keys = await env.DALI_FEEDBACK.list({ prefix: "fb:", limit: 100 });
        const items = await Promise.all(keys.keys.map(k => env.DALI_FEEDBACK.get(k.name, "json")));
        list = items.filter(Boolean);
        list.sort((a, b) => b.timestamp - a.timestamp);
      } catch (_) {
        list = [...MEMORY_FEEDBACK_CACHE];
      }
    } else {
      list = [...MEMORY_FEEDBACK_CACHE];
    }

    // 渲染漂亮的看板表格页面
    const rowsHtml = list.length === 0 
      ? `<tr><td colspan="7" style="text-align:center;padding:40px;color:#94a3b8;">暂无用户反馈记录</td></tr>`
      : list.map((item, idx) => {
          let badgeColor = "#2563eb";
          if (item.type === "crash" || item.type === "bug") badgeColor = "#ef4444";
          if (item.type === "table") badgeColor = "#f59e0b";
          if (item.type === "feature") badgeColor = "#10b981";

          return `
            <tr>
              <td style="color:#94a3b8;font-size:12px;white-space:nowrap;">${item.time || "-"}</td>
              <td><span style="display:inline-block;padding:2px 8px;border-radius:4px;font-size:12px;background:${badgeColor}22;color:${badgeColor};border:1px solid ${badgeColor}44;">${item.type}</span></td>
              <td style="font-weight:600;color:#f8fafc;white-space:nowrap;">
                ${escapeHtml(item.contact)}
                ${item.contact && item.contact !== "未留联系方式" ? `<button type="button" onclick="navigator.clipboard.writeText('${escapeHtml(item.contact)}');alert('已复制联系方式: ${escapeHtml(item.contact)}')" style="background:transparent;border:none;cursor:pointer;font-size:11px;margin-left:4px;" title="复制联系方式">📋</button>` : ""}
              </td>
              <td style="color:#cbd5e1;font-size:12.5px;white-space:nowrap;">v${escapeHtml(item.client.version)} <span style="opacity:0.6;">(${escapeHtml(item.client.platform)}-${escapeHtml(item.client.arch)})</span></td>
              <td style="line-height:1.6;color:#f1f5f9;min-width:240px;">${escapeHtml(item.content)}</td>
              <td>
                ${item.error_log ? `<details style="font-size:11px;color:#f87171;"><summary style="cursor:pointer;">查看报错日志</summary><pre style="margin-top:6px;background:#0f172a;padding:8px;border-radius:6px;max-width:300px;overflow:auto;">${escapeHtml(item.error_log)}</pre></details>` : `<span style="color:#64748b;font-size:12px;">无异常</span>`}
              </td>
              <td style="white-space:nowrap;">
                <a href="/admin/delete?id=${item.id}&key=${providedKey}" onclick="return confirm('确定删除这条反馈吗？')" style="color:#ef4444;text-decoration:none;font-size:12px;padding:3px 8px;background:rgba(239,68,68,0.12);border-radius:4px;border:1px solid rgba(239,68,68,0.3);">🗑️ 删除</a>
              </td>
            </tr>
          `;
        }).join("");

    return htmlResponse(`
      <!DOCTYPE html>
      <html lang="zh-CN">
      <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>大梨 OCR · 用户反馈与意见管理看板</title>
        <style>
          body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0b0f19; color: #f8fafc; margin: 0; padding: 24px 32px; }
          .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; padding-bottom: 16px; border-bottom: 1px solid #1e293b; }
          .title { font-size: 22px; font-weight: 700; display: flex; align-items: center; gap: 10px; }
          .stats { display: flex; gap: 16px; align-items: center; }
          .stat-pill { background: #1e293b; padding: 6px 14px; border-radius: 20px; font-size: 13px; border: 1px solid #334155; }
          .btn { background: #2563eb; color: #fff; border: none; padding: 8px 16px; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; text-decoration: none; }
          .btn:hover { background: #1d4ed8; }
          .card { background: #131b2e; border: 1px solid #1e293b; border-radius: 12px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.3); }
          table { width: 100%; border-collapse: collapse; text-align: left; }
          th { background: #182238; color: #94a3b8; font-weight: 600; font-size: 12.5px; padding: 12px 16px; border-bottom: 1px solid #1e293b; }
          td { padding: 14px 16px; border-bottom: 1px solid #1e293b; font-size: 13.5px; }
          tr:last-child td { border-bottom: none; }
          tr:hover td { background: rgba(255,255,255,0.02); }
        </style>
      </head>
      <body>
        <div class="header">
          <div class="title">🍐 大梨 OCR · 用户意见与报错管理看板</div>
          <div class="stats">
            <div class="stat-pill">反馈总数: <b>${list.length}</b> 条</div>
            <a href="/admin?key=${providedKey}" class="btn">🔄 刷新数据</a>
          </div>
        </div>

        <div class="card">
          <table>
            <thead>
              <tr>
                <th>提交时间</th>
                <th>类型</th>
                <th>用户联系方式</th>
                <th>客户端环境</th>
                <th>反馈具体内容</th>
                <th>崩溃/异常堆栈</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              ${rowsHtml}
            </tbody>
          </table>
        </div>
      </body>
      </html>
    `);
  },

  // 5. 开发者删除单条反馈记录
  async handleAdminDelete(request, url, env) {
    const adminKey = env?.ADMIN_KEY || CONFIG.admin.defaultKey;
    const providedKey = url.searchParams.get("key");
    const targetId = url.searchParams.get("id");

    if (providedKey !== adminKey) {
      return jsonResponse({ code: 401, msg: "未授权操作" }, 401);
    }

    if (targetId) {
      // 从内存缓存中删除
      const memIdx = MEMORY_FEEDBACK_CACHE.findIndex(i => i.id === targetId);
      if (memIdx !== -1) MEMORY_FEEDBACK_CACHE.splice(memIdx, 1);

      // 从 Cloudflare KV 中删除
      if (env?.DALI_FEEDBACK) {
        try {
          const keys = await env.DALI_FEEDBACK.list({ prefix: "fb:", limit: 100 });
          for (const k of keys.keys) {
            if (k.name.endsWith(`:${targetId}`)) {
              await env.DALI_FEEDBACK.delete(k.name);
            }
          }
        } catch (_) {}
      }
    }

    // 重定向回看板
    return Response.redirect(`${url.origin}/admin?key=${providedKey}`, 302);
  },

  // 6. 支付平台自动发卡/回调预留
  async handlePayWebhook(request, url, env) {
    try {
      const payload = await request.json();
      console.log("收到支付平台 Webhook 回调通知:", payload);
      return jsonResponse({ code: 0, msg: "webhook received", status: "success" });
    } catch (_) {
      return jsonResponse({ code: 4000, msg: "invalid payload" }, 400);
    }
  }
};

function escapeHtml(str) {
  if (!str) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// ============================================================================
// 4. Cloudflare Worker 主入口
// ============================================================================
export default {
  async fetch(request, env, ctx) {
    if (request.method === "OPTIONS") {
      return new Response(null, {
        status: 204,
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Requested-With",
          "Access-Control-Max-Age": "86400"
        }
      });
    }

    const url = new URL(request.url);
    const pathname = url.pathname.replace(/\/+$/, "") || "/";

    try {
      if (pathname === "/" || pathname === "/api") {
        return await handlers.handleRoot(request);
      }
      if (pathname === "/update" || pathname === "/api/update") {
        return await handlers.handleUpdate(request, url);
      }
      if (pathname === "/notice" || pathname === "/api/notice") {
        return await handlers.handleNotice(request);
      }
      if (pathname === "/feedback" || pathname === "/api/feedback") {
        return await handlers.handleFeedback(request, url, env);
      }
      if (pathname === "/admin" || pathname === "/admin/feedback") {
        return await handlers.handleAdmin(request, url, env);
      }
      if (pathname === "/admin/delete") {
        return await handlers.handleAdminDelete(request, url, env);
      }
      if (pathname === "/pay/webhook" || pathname === "/api/pay/webhook") {
        return await handlers.handlePayWebhook(request, url, env);
      }

      return jsonResponse({
        code: 404,
        msg: `Endpoint not found: ${pathname}`,
        help: "Visit / for API documentation"
      }, 404);

    } catch (err) {
      console.error("Worker Execution Error:", err);
      return jsonResponse({
        code: 500,
        msg: "Internal Server Error",
        error: err.message
      }, 500);
    }
  }
};
