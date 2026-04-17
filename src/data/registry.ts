export interface EnvVarSpec {
  name: string;
  description: string;
  required: boolean;
}

export type RegistryRegion = "international" | "china";

export interface RegistryEntry {
  id: string;
  name: string;
  /** Publisher/maintainer shown on card (e.g. "Anthropic", "GitHub", "高德开放平台") */
  publisher: string;
  description: string;
  region: RegistryRegion;
  tags: string[];
  command: string;
  args: string[];
  envVars: EnvVarSpec[];
  sourceUrl: string;
  icon?: string; // emoji
}

export const REGISTRY_ENTRIES: RegistryEntry[] = [
  // ==================== INTERNATIONAL ====================
  {
    id: "filesystem",
    name: "Filesystem",
    publisher: "Anthropic",
    description: "Read, write, and search files in configured directories.",
    region: "international",
    tags: ["files", "local"],
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/dir"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem",
    icon: "📁",
  },
  {
    id: "fetch",
    name: "Fetch",
    publisher: "Anthropic",
    description: "Fetch web content for LLM consumption (HTML → markdown).",
    region: "international",
    tags: ["web", "http"],
    command: "uvx",
    args: ["mcp-server-fetch"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/fetch",
    icon: "🌐",
  },
  {
    id: "memory",
    name: "Memory",
    publisher: "Anthropic",
    description: "Persistent knowledge graph memory across conversations.",
    region: "international",
    tags: ["memory", "graph"],
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
    envVars: [
      {
        name: "MEMORY_FILE_PATH",
        description: "Path to the memory file (optional, defaults to in-app)",
        required: false,
      },
    ],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/memory",
    icon: "🧠",
  },
  {
    id: "sequential-thinking",
    name: "Sequential Thinking",
    publisher: "Anthropic",
    description: "Structured problem-solving through step-by-step reasoning.",
    region: "international",
    tags: ["reasoning"],
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-sequential-thinking"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking",
    icon: "🤔",
  },
  {
    id: "git",
    name: "Git",
    publisher: "Anthropic",
    description: "Read, search, and manipulate Git repositories.",
    region: "international",
    tags: ["dev", "vcs"],
    command: "uvx",
    args: ["mcp-server-git", "--repository", "/path/to/repo"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/git",
    icon: "🔀",
  },
  {
    id: "github",
    name: "GitHub",
    publisher: "GitHub",
    description: "Access GitHub repos, issues, PRs, and code search.",
    region: "international",
    tags: ["dev", "vcs", "github"],
    command: "npx",
    args: ["-y", "@github/github-mcp-server"],
    envVars: [
      {
        name: "GITHUB_PERSONAL_ACCESS_TOKEN",
        description: "GitHub PAT with repo + read:org scopes",
        required: true,
      },
    ],
    sourceUrl: "https://github.com/github/github-mcp-server",
    icon: "🐙",
  },
  {
    id: "postgres",
    name: "Postgres",
    publisher: "Anthropic",
    description: "Query PostgreSQL databases with read-only access.",
    region: "international",
    tags: ["database", "sql"],
    command: "npx",
    args: [
      "-y",
      "@modelcontextprotocol/server-postgres",
      "postgresql://user:pass@host/db",
    ],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/postgres",
    icon: "🐘",
  },
  {
    id: "sqlite",
    name: "SQLite",
    publisher: "Anthropic",
    description: "Query and manipulate SQLite databases.",
    region: "international",
    tags: ["database", "sql"],
    command: "uvx",
    args: ["mcp-server-sqlite", "--db-path", "/path/to/db.sqlite"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/sqlite",
    icon: "💾",
  },
  {
    id: "brave-search",
    name: "Brave Search",
    publisher: "Brave",
    description: "Web search via Brave's privacy-focused search engine.",
    region: "international",
    tags: ["search", "web"],
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-brave-search"],
    envVars: [
      {
        name: "BRAVE_API_KEY",
        description: "Brave Search API key (free tier available)",
        required: true,
      },
    ],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/brave-search",
    icon: "🦁",
  },
  {
    id: "notion",
    name: "Notion",
    publisher: "Notion",
    description: "Read and write Notion pages, databases, and comments.",
    region: "international",
    tags: ["productivity", "docs"],
    command: "npx",
    args: ["-y", "@notionhq/notion-mcp-server"],
    envVars: [
      {
        name: "NOTION_TOKEN",
        description: "Notion integration token",
        required: true,
      },
    ],
    sourceUrl: "https://github.com/makenotion/notion-mcp-server",
    icon: "📓",
  },
  {
    id: "playwright",
    name: "Playwright",
    publisher: "Microsoft",
    description: "Browser automation — navigate, click, and extract page data.",
    region: "international",
    tags: ["browser", "automation"],
    command: "npx",
    args: ["-y", "@playwright/mcp@latest"],
    envVars: [],
    sourceUrl: "https://github.com/microsoft/playwright-mcp",
    icon: "🎭",
  },
  {
    id: "context7",
    name: "Context7",
    publisher: "Upstash",
    description: "Up-to-date library docs for LLMs (React, Next.js, etc.).",
    region: "international",
    tags: ["docs", "dev"],
    command: "npx",
    args: ["-y", "@upstash/context7-mcp"],
    envVars: [],
    sourceUrl: "https://github.com/upstash/context7",
    icon: "📚",
  },
  {
    id: "supabase",
    name: "Supabase",
    publisher: "Supabase",
    description: "Manage Supabase projects, databases, and auth.",
    region: "international",
    tags: ["database", "backend"],
    command: "npx",
    args: ["-y", "@supabase/mcp-server-supabase"],
    envVars: [
      {
        name: "SUPABASE_ACCESS_TOKEN",
        description: "Supabase personal access token",
        required: true,
      },
    ],
    sourceUrl: "https://github.com/supabase-community/supabase-mcp",
    icon: "⚡",
  },
  {
    id: "stripe",
    name: "Stripe",
    publisher: "Stripe",
    description: "Access Stripe payments, customers, and products.",
    region: "international",
    tags: ["payments", "api"],
    command: "npx",
    args: ["-y", "@stripe/mcp"],
    envVars: [
      {
        name: "STRIPE_SECRET_KEY",
        description: "Stripe secret key (sk_live_... or sk_test_...)",
        required: true,
      },
    ],
    sourceUrl: "https://github.com/stripe/agent-toolkit",
    icon: "💳",
  },
  {
    id: "everything",
    name: "Everything",
    publisher: "Anthropic",
    description: "Demo server showcasing all MCP capabilities (tools, prompts, resources).",
    region: "international",
    tags: ["demo", "dev"],
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-everything"],
    envVars: [],
    sourceUrl:
      "https://github.com/modelcontextprotocol/servers/tree/main/src/everything",
    icon: "✨",
  },

  // ==================== CHINA ====================
  {
    id: "amap-maps",
    name: "高德地图",
    publisher: "高德开放平台",
    description: "高德地图服务：地理编码、路径规划、POI 搜索、天气查询。",
    region: "china",
    tags: ["地图", "位置"],
    command: "npx",
    args: ["-y", "@amap/amap-maps-mcp-server"],
    envVars: [
      {
        name: "AMAP_MAPS_API_KEY",
        description: "高德开放平台 API Key",
        required: true,
      },
    ],
    sourceUrl: "https://lbs.amap.com/api/mcp-server/summary",
    icon: "🗺️",
  },
  {
    id: "baidu-maps",
    name: "百度地图",
    publisher: "百度地图开放平台",
    description: "百度地图服务：定位、路径规划、POI 检索。",
    region: "china",
    tags: ["地图", "位置"],
    command: "npx",
    args: ["-y", "@baidumap/mcp-server-baidu-map"],
    envVars: [
      {
        name: "BAIDU_MAP_API_KEY",
        description: "百度地图开放平台 AK",
        required: true,
      },
    ],
    sourceUrl: "https://lbsyun.baidu.com/",
    icon: "🐾",
  },
  {
    id: "tencent-lbs",
    name: "腾讯位置服务",
    publisher: "腾讯位置服务",
    description: "腾讯位置服务：地理编码、路径规划、周边搜索。",
    region: "china",
    tags: ["地图", "位置"],
    command: "npx",
    args: ["-y", "@tencent-lbs/mcp-server"],
    envVars: [
      {
        name: "TENCENT_LBS_KEY",
        description: "腾讯位置服务 Key",
        required: true,
      },
    ],
    sourceUrl: "https://lbs.qq.com/",
    icon: "🐧",
  },
  {
    id: "dingtalk",
    name: "钉钉",
    publisher: "钉钉 MCP 广场",
    description: "钉钉：文档、日历、联系人、表格管理。",
    region: "china",
    tags: ["办公", "协作"],
    command: "npx",
    args: ["-y", "@dingtalk/mcp-server"],
    envVars: [
      {
        name: "DINGTALK_APP_KEY",
        description: "钉钉应用 AppKey",
        required: true,
      },
      {
        name: "DINGTALK_APP_SECRET",
        description: "钉钉应用 AppSecret",
        required: true,
      },
    ],
    sourceUrl: "https://mcp.dingtalk.com/",
    icon: "💼",
  },
  {
    id: "tencent-cos",
    name: "腾讯 COS",
    publisher: "腾讯云 CloudBase",
    description: "腾讯云对象存储：文件上传、下载、桶管理。",
    region: "china",
    tags: ["存储", "云"],
    command: "npx",
    args: ["-y", "@tencent-cos/mcp-server"],
    envVars: [
      {
        name: "COS_SECRET_ID",
        description: "腾讯云 API SecretId",
        required: true,
      },
      {
        name: "COS_SECRET_KEY",
        description: "腾讯云 API SecretKey",
        required: true,
      },
    ],
    sourceUrl: "https://tcb.cloud.tencent.com/mcp-server",
    icon: "☁️",
  },
  {
    id: "tencent-hunyuan-3d",
    name: "腾讯混元 3D",
    publisher: "腾讯云 CloudBase",
    description: "腾讯混元 3D 生成:文本/图像转 3D 模型。",
    region: "china",
    tags: ["AI", "3D"],
    command: "npx",
    args: ["-y", "@tencent/hunyuan-3d-mcp-server"],
    envVars: [
      {
        name: "TENCENT_CLOUD_SECRET_ID",
        description: "腾讯云 SecretId",
        required: true,
      },
      {
        name: "TENCENT_CLOUD_SECRET_KEY",
        description: "腾讯云 SecretKey",
        required: true,
      },
    ],
    sourceUrl: "https://tcb.cloud.tencent.com/mcp-server",
    icon: "🎨",
  },
  {
    id: "alipay",
    name: "支付宝",
    publisher: "阿里云百炼",
    description: "支付宝开放平台:支付、转账、账单查询。",
    region: "china",
    tags: ["支付", "金融"],
    command: "npx",
    args: ["-y", "@alipay/mcp-server"],
    envVars: [
      {
        name: "ALIPAY_APP_ID",
        description: "支付宝应用 AppId",
        required: true,
      },
      {
        name: "ALIPAY_PRIVATE_KEY",
        description: "支付宝应用私钥",
        required: true,
      },
    ],
    sourceUrl: "https://bailian.console.aliyun.com/",
    icon: "💰",
  },
  {
    id: "baidu-netdisk",
    name: "百度网盘",
    publisher: "百度千帆",
    description: "百度网盘:文件管理、分享、搜索。",
    region: "china",
    tags: ["存储", "云"],
    command: "npx",
    args: ["-y", "@baidu/netdisk-mcp-server"],
    envVars: [
      {
        name: "BAIDU_NETDISK_ACCESS_TOKEN",
        description: "百度网盘 OAuth access token",
        required: true,
      },
    ],
    sourceUrl: "https://qianfan.cloud.baidu.com/",
    icon: "📦",
  },
];

export function filterEntries(
  region: RegistryRegion,
  query: string,
): RegistryEntry[] {
  const lowerQuery = query.toLowerCase().trim();
  return REGISTRY_ENTRIES.filter((entry) => {
    if (entry.region !== region) return false;
    if (!lowerQuery) return true;
    return (
      entry.name.toLowerCase().includes(lowerQuery) ||
      entry.description.toLowerCase().includes(lowerQuery) ||
      entry.publisher.toLowerCase().includes(lowerQuery) ||
      entry.tags.some((t) => t.toLowerCase().includes(lowerQuery)) ||
      entry.id.toLowerCase().includes(lowerQuery)
    );
  });
}
