import type english from '@/i18n/locales/en.json';

export type McpDescriptionKey = Extract<keyof typeof english, `mcpDescription${string}`>;
export type McpCategoryKey = Extract<keyof typeof english, `mcpCategory${string}`>;

export type McpCategory =
  | 'developer'
  | 'database'
  | 'webSearch'
  | 'devOpsCloud'
  | 'productivity'
  | 'memoryReasoning'
  | 'finance';

export interface McpServerRecord {
  id: string;
  name: string;
  category: McpCategory;
  description: string;
  descriptionKey?: McpDescriptionKey;
  command: string;
  args: string[];
  requiredEnvKeys: string[];
  optionalEnvKeys: string[];
  installed: boolean;
  enabled: boolean;
  installDir: string;
  discoveredTools: string[];
}

type CuratedMcpServer = Pick<
  McpServerRecord,
  'id' | 'name' | 'category' | 'descriptionKey' | 'command' | 'args' | 'requiredEnvKeys' | 'discoveredTools'
>;

export const popularMcpCatalog: readonly CuratedMcpServer[] = [
  {
    id: 'github', name: 'GitHub', category: 'developer', descriptionKey: 'mcpDescriptionGithub',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-github'], requiredEnvKeys: ['GITHUB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: [
      'create_or_update_file: Dosya oluştur veya güncelle',
      'search_repositories: GitHub depolarında arama yap',
      'create_issue: Yeni issue aç',
      'get_issue: Issue ayrıntılarını oku',
      'list_pull_requests: Pull request listele',
      'create_pull_request: Yeni PR oluştur',
    ],
  },
  {
    id: 'git', name: 'Git', category: 'developer', descriptionKey: 'mcpDescriptionGit',
    command: 'uvx', args: ['mcp-server-git'], requiredEnvKeys: [],
    discoveredTools: [
      'git_status: Çalışma dizini durumunu oku',
      'git_diff_unstaged: Değişiklik farklarını göster',
      'git_log: Commit geçmişini listele',
      'git_create_branch: Yeni dal oluştur',
    ],
  },
  {
    id: 'gitlab', name: 'GitLab', category: 'developer', descriptionKey: 'mcpDescriptionGitlab',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-gitlab'], requiredEnvKeys: ['GITLAB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: [
      'list_projects: Projeleri listele',
      'get_merge_request: MR ayrıntılarını getir',
      'create_issue: GitLab issue oluştur',
    ],
  },
  {
    id: 'sentry', name: 'Sentry', category: 'developer', descriptionKey: 'mcpDescriptionSentry',
    command: 'uvx', args: ['mcp-server-sentry'], requiredEnvKeys: ['SENTRY_AUTH_TOKEN'],
    discoveredTools: [
      'get_issue: Hata detaylarını ve yığın izini getir',
      'search_events: Hata olaylarında arama yap',
    ],
  },
  {
    id: 'linear', name: 'Linear', category: 'developer', descriptionKey: 'mcpDescriptionLinear',
    command: 'npx', args: ['-y', 'linear-mcp'], requiredEnvKeys: ['LINEAR_API_KEY'],
    discoveredTools: [
      'list_issues: Görevleri listele',
      'create_issue: Yeni Linear görevi oluştur',
      'update_issue: Görev durumunu güncelle',
    ],
  },
  {
    id: 'postgres', name: 'PostgreSQL', category: 'database', descriptionKey: 'mcpDescriptionPostgres',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-postgres'], requiredEnvKeys: ['DATABASE_URL'],
    discoveredTools: [
      'query: Salt-okunur SQL sorgusu çalıştır',
      'describe_table: Tablo şemasını ve sütun tiplerini göster',
      'list_tables: Tüm tabloları listele',
    ],
  },
  {
    id: 'sqlite', name: 'SQLite', category: 'database', descriptionKey: 'mcpDescriptionSqlite',
    command: 'uvx', args: ['mcp-server-sqlite'], requiredEnvKeys: ['SQLITE_DB_PATH'],
    discoveredTools: [
      'read_query: SELECT sorgusu koştur',
      'list_tables: Tabloları listele',
      'describe_table: Tablo yapısını incele',
    ],
  },
  {
    id: 'supabase', name: 'Supabase', category: 'database', descriptionKey: 'mcpDescriptionSupabase',
    command: 'npx', args: ['-y', '@supabase/mcp-server'], requiredEnvKeys: ['SUPABASE_ACCESS_TOKEN'],
    discoveredTools: [
      'execute_sql: SQL sorgusu çalıştır',
      'list_buckets: Depolama klasörlerini listele',
      'get_project_status: Proje durumunu kontrol et',
    ],
  },
  {
    id: 'redis', name: 'Redis', category: 'database', descriptionKey: 'mcpDescriptionRedis',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-redis'], requiredEnvKeys: ['REDIS_URL'],
    discoveredTools: [
      'get_key: Anahtar değerini oku',
      'list_keys: Anahtar desenlerini listele',
    ],
  },
  {
    id: 'brave_search', name: 'Brave Search', category: 'webSearch', descriptionKey: 'mcpDescriptionBraveSearch',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-brave-search'], requiredEnvKeys: ['BRAVE_API_KEY'],
    discoveredTools: [
      'brave_web_search: Web araması yap ve özet sonuçları getir',
      'brave_local_search: Yerel işletme ve konum araması yap',
    ],
  },
  {
    id: 'fetch', name: 'Fetch', category: 'webSearch', descriptionKey: 'mcpDescriptionFetch',
    command: 'uvx', args: ['mcp-server-fetch'], requiredEnvKeys: [],
    discoveredTools: ['fetch_url: URL içeriğini temiz Markdown formatında çek'],
  },
  {
    id: 'puppeteer', name: 'Puppeteer', category: 'webSearch', descriptionKey: 'mcpDescriptionPuppeteer',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-puppeteer'], requiredEnvKeys: [],
    discoveredTools: [
      'navigate: Sayfaya git',
      'click: Öğeye tıkla',
      'fill: Form alanını doldur',
      'screenshot: Ekran görüntüsü al',
    ],
  },
  {
    id: 'playwright', name: 'Playwright', category: 'webSearch', descriptionKey: 'mcpDescriptionPlaywright',
    command: 'npx', args: ['-y', '@executeautomation/playwright-mcp-server'], requiredEnvKeys: [],
    discoveredTools: [
      'playwright_navigate: Belirtilen URL adresine git',
      'playwright_screenshot: Ekran görüntüsü kaydet',
      'playwright_click: Buton veya bağlantıya tıkla',
    ],
  },
  {
    id: 'firecrawl', name: 'Firecrawl', category: 'webSearch', descriptionKey: 'mcpDescriptionFirecrawl',
    command: 'npx', args: ['-y', 'firecrawl-mcp'], requiredEnvKeys: ['FIRECRAWL_API_KEY'],
    discoveredTools: [
      'scrape_url: Sayfayı temiz metin olarak kazı',
      'crawl_domain: Tüm siteyi haritalandırıp tara',
    ],
  },
  {
    id: 'docker', name: 'Docker', category: 'devOpsCloud', descriptionKey: 'mcpDescriptionDocker',
    command: 'uvx', args: ['mcp-server-docker'], requiredEnvKeys: [],
    discoveredTools: [
      'list_containers: Çalışan ve durdurulan konteynerleri göster',
      'get_container_logs: Konteyner loglarını oku',
      'restart_container: Konteyneri yeniden başlat',
    ],
  },
  {
    id: 'cloudflare', name: 'Cloudflare', category: 'devOpsCloud', descriptionKey: 'mcpDescriptionCloudflare',
    command: 'npx', args: ['-y', '@cloudflare/mcp-server-cloudflare'], requiredEnvKeys: ['CLOUDFLARE_API_TOKEN'],
    discoveredTools: [
      'list_workers: Workers betiklerini listele',
      'query_d1: D1 veritabanı sorgula',
    ],
  },
  {
    id: 'kubernetes', name: 'Kubernetes', category: 'devOpsCloud', descriptionKey: 'mcpDescriptionKubernetes',
    command: 'uvx', args: ['mcp-server-kubernetes'], requiredEnvKeys: [],
    discoveredTools: [
      'list_pods: Pod durumlarını listele',
      'get_pod_logs: Pod loglarını getir',
      'describe_resource: Kaynak detaylarını incele',
    ],
  },
  {
    id: 'google_drive', name: 'Google Drive', category: 'productivity', descriptionKey: 'mcpDescriptionGoogleDrive',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-gdrive'], requiredEnvKeys: ['GOOGLE_APPLICATION_CREDENTIALS'],
    discoveredTools: [
      'search_files: Drive dosyalarında arama yap',
      'read_file: Belge içeriğini oku',
    ],
  },
  {
    id: 'notion', name: 'Notion', category: 'productivity', descriptionKey: 'mcpDescriptionNotion',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-notion'], requiredEnvKeys: ['NOTION_API_KEY'],
    discoveredTools: [
      'search_pages: Sayfa ve veritabanı ara',
      'get_page_content: Sayfa bloklarını oku',
      'append_block: Sayfaya yeni içerik ekle',
    ],
  },
  {
    id: 'slack', name: 'Slack', category: 'productivity', descriptionKey: 'mcpDescriptionSlack',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-slack'], requiredEnvKeys: ['SLACK_BOT_TOKEN'],
    discoveredTools: [
      'list_channels: Kanalları listele',
      'get_channel_history: Mesaj geçmişini oku',
      'post_message: Kanala mesaj gönder',
    ],
  },
  {
    id: 'discord', name: 'Discord', category: 'productivity', descriptionKey: 'mcpDescriptionDiscord',
    command: 'npx', args: ['-y', 'mcp-server-discord'], requiredEnvKeys: ['DISCORD_BOT_TOKEN'],
    discoveredTools: [
      'list_guild_channels: Sunucu kanallarını listele',
      'send_message: Kanala mesaj gönder',
    ],
  },
  {
    id: 'obsidian', name: 'Obsidian', category: 'productivity', descriptionKey: 'mcpDescriptionObsidian',
    command: 'npx', args: ['-y', 'mcp-obsidian'], requiredEnvKeys: ['OBSIDIAN_VAULT_PATH'],
    discoveredTools: [
      'search_notes: Notlarda arama yap',
      'read_note: Not içeriğini oku',
    ],
  },
  {
    id: 'filesystem', name: 'Filesystem', category: 'productivity', descriptionKey: 'mcpDescriptionFilesystem',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-filesystem'], requiredEnvKeys: ['MCP_ALLOWED_DIRECTORY'],
    discoveredTools: [
      'read_file: Dosya oku',
      'write_file: Dosyaya yaz',
      'list_directory: Dizin listele',
    ],
  },
  {
    id: 'memory', name: 'Memory Graph', category: 'memoryReasoning', descriptionKey: 'mcpDescriptionMemory',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-memory'], requiredEnvKeys: [],
    discoveredTools: [
      'create_entities: Varlıkları hafızaya kaydet',
      'create_relations: Varlıklar arası ilişki kur',
      'read_graph: Bilgi grafını sorgula',
    ],
  },
  {
    id: 'sequential_thinking', name: 'Sequential Thinking', category: 'memoryReasoning', descriptionKey: 'mcpDescriptionSequentialThinking',
    command: 'npx', args: ['-y', '@modelcontextprotocol/server-sequential-thinking'], requiredEnvKeys: [],
    discoveredTools: ['sequential_thinking: Çok adımlı mantıksal düşünce süreci yürüt'],
  },
  {
    id: 'time', name: 'Time & Timezone', category: 'memoryReasoning', descriptionKey: 'mcpDescriptionTime',
    command: 'uvx', args: ['mcp-server-time'], requiredEnvKeys: [],
    discoveredTools: [
      'get_current_time: Geçerli zamanı al',
      'convert_time: Saat dilimleri arasında dönüşüm yap',
    ],
  },
  {
    id: 'stripe', name: 'Stripe', category: 'finance', descriptionKey: 'mcpDescriptionStripe',
    command: 'npx', args: ['-y', 'stripe-mcp-server'], requiredEnvKeys: ['STRIPE_SECRET_KEY'],
    discoveredTools: [
      'search_customers: Müşteri ara',
      'get_subscription: Abonelik detaylarını oku',
      'list_invoices: Faturaları listele',
    ],
  },
];

export function createMcpCatalog(): McpServerRecord[] {
  return popularMcpCatalog.map((server) => ({
    ...server,
    description: '',
    optionalEnvKeys: [],
    installed: false,
    enabled: false,
    installDir: '',
    discoveredTools: [...server.discoveredTools],
  }));
}
