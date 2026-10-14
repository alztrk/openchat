import 'package:openchat/features/mcp/domain/global_mcp_server.dart';

/// Predefined popular MCP servers curated for the OpenChat Library.
/// (OpenMail is intentionally excluded).
const List<GlobalMcpServer> popularMcpCatalog = [
  // 1. Developer Tools
  GlobalMcpServer(
    id: 'github',
    name: 'GitHub',
    category: 'Developer',
    description:
        'GitHub API entegrasyonu: Depo arama, dosya okuma/yazma, issue ve pull request inceleme ve oluşturma.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    requiredEnvKeys: ['GITHUB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: [
      'create_or_update_file: Dosya oluştur veya güncelle',
      'search_repositories: GitHub depolarında arama yap',
      'create_issue: Yeni issue aç',
      'get_issue: Issue ayrıntılarını oku',
      'list_pull_requests: Pull request listele',
      'create_pull_request: Yeni PR oluştur',
    ],
  ),
  GlobalMcpServer(
    id: 'git',
    name: 'Git',
    category: 'Developer',
    description:
        'Yerel Git depolarında diff, log, status, branch ve commit işlemlerini güvenle çalıştırma.',
    command: 'uvx',
    args: ['mcp-server-git'],
    requiredEnvKeys: [],
    discoveredTools: [
      'git_status: Çalışma dizini durumunu oku',
      'git_diff_unstaged: Değişiklik farklarını göster',
      'git_log: Commit geçmişini listele',
      'git_create_branch: Yeni dal oluştur',
    ],
  ),
  GlobalMcpServer(
    id: 'gitlab',
    name: 'GitLab',
    category: 'Developer',
    description:
        'GitLab projeleri, merge requestler, issue takibi ve pipeline kontrolleri.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-gitlab'],
    requiredEnvKeys: ['GITLAB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: [
      'list_projects: Projeleri listele',
      'get_merge_request: MR ayrıntılarını getir',
      'create_issue: GitLab issue oluştur',
    ],
  ),
  GlobalMcpServer(
    id: 'sentry',
    name: 'Sentry',
    category: 'Developer',
    description:
        'Canlı hata kayıtlarını, stack trace analizlerini ve performans metriklerini inceleme.',
    command: 'uvx',
    args: ['mcp-server-sentry'],
    requiredEnvKeys: ['SENTRY_AUTH_TOKEN'],
    discoveredTools: [
      'get_issue: Hata detaylarını ve yığın izini getir',
      'search_events: Hata olaylarında arama yap',
    ],
  ),
  GlobalMcpServer(
    id: 'linear',
    name: 'Linear',
    category: 'Developer',
    description:
        'Linear görevlerini, sprintleri ve proje issue durumlarını yönetme.',
    command: 'npx',
    args: ['-y', 'linear-mcp'],
    requiredEnvKeys: ['LINEAR_API_KEY'],
    discoveredTools: [
      'list_issues: Görevleri listele',
      'create_issue: Yeni Linear görevi oluştur',
      'update_issue: Görev durumunu güncelle',
    ],
  ),

  // 2. Databases & Storage
  GlobalMcpServer(
    id: 'postgres',
    name: 'PostgreSQL',
    category: 'Database',
    description:
        'Postgres veritabanına bağlanıp tabloları, şemaları inceleme ve salt-okunur SQL sorgulama.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-postgres'],
    requiredEnvKeys: ['DATABASE_URL'],
    discoveredTools: [
      'query: Salt-okunur SQL sorgusu çalıştır',
      'describe_table: Tablo şemasını ve sütun tiplerini göster',
      'list_tables: Tüm tabloları listele',
    ],
  ),
  GlobalMcpServer(
    id: 'sqlite',
    name: 'SQLite',
    category: 'Database',
    description:
        'Yerel SQLite veritabanı dosyalarını açıp şema analizi ve analitik sorgular koşturma.',
    command: 'uvx',
    args: ['mcp-server-sqlite'],
    requiredEnvKeys: ['SQLITE_DB_PATH'],
    discoveredTools: [
      'read_query: SELECT sorgusu koştur',
      'list_tables: Tabloları listele',
      'describe_table: Tablo yapısını incele',
    ],
  ),
  GlobalMcpServer(
    id: 'supabase',
    name: 'Supabase',
    category: 'Database',
    description:
        'Supabase veritabanı, auth kuralları, depolama ve edge functions yönetimi.',
    command: 'npx',
    args: ['-y', '@supabase/mcp-server'],
    requiredEnvKeys: ['SUPABASE_ACCESS_TOKEN'],
    discoveredTools: [
      'execute_sql: SQL sorgusu çalıştır',
      'list_buckets: Depolama klasörlerini listele',
      'get_project_status: Proje durumunu kontrol et',
    ],
  ),
  GlobalMcpServer(
    id: 'redis',
    name: 'Redis',
    category: 'Database',
    description:
        'Redis anahtarlarını, TTL sürelerini, bellek kullanımını ve veri tiplerini sorgulama.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-redis'],
    requiredEnvKeys: ['REDIS_URL'],
    discoveredTools: [
      'get_key: Anahtar değerini oku',
      'list_keys: Anahtar desenlerini listele',
    ],
  ),

  // 3. Web & Search
  GlobalMcpServer(
    id: 'brave_search',
    name: 'Brave Search',
    category: 'Web & Search',
    description:
        'Brave Search motoru ile yapay zekaya güncel internet araması ve yerel sonuç yeteneği kazandırma.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-brave-search'],
    requiredEnvKeys: ['BRAVE_API_KEY'],
    discoveredTools: [
      'brave_web_search: Web araması yap ve özet sonuçları getir',
      'brave_local_search: Yerel işletme ve konum araması yap',
    ],
  ),
  GlobalMcpServer(
    id: 'fetch',
    name: 'Fetch',
    category: 'Web & Search',
    description:
        'Web sayfalarını indirip modelin rahatça analiz edebileceği temiz Markdown ve metne dönüştürme.',
    command: 'uvx',
    args: ['mcp-server-fetch'],
    requiredEnvKeys: [],
    discoveredTools: [
      'fetch_url: URL içeriğini temiz Markdown formatında çek',
    ],
  ),
  GlobalMcpServer(
    id: 'puppeteer',
    name: 'Puppeteer',
    category: 'Web & Search',
    description:
        'Otonom Chromium tarayıcısıyla sayfa tıklama, form doldurma ve tam sayfa ekran görüntüsü alma.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-puppeteer'],
    requiredEnvKeys: [],
    discoveredTools: [
      'navigate: Sayfaya git',
      'click: Öğeye tıkla',
      'fill: Form alanını doldur',
      'screenshot: Ekran görüntüsü al',
    ],
  ),
  GlobalMcpServer(
    id: 'playwright',
    name: 'Playwright',
    category: 'Web & Search',
    description:
        'Gelişmiş çapraz tarayıcı otomasyonu, kullanıcı akışları ve uçtan uca UI testleri.',
    command: 'npx',
    args: ['-y', '@executeautomation/playwright-mcp-server'],
    requiredEnvKeys: [],
    discoveredTools: [
      'playwright_navigate: Belirtilen URL adresine git',
      'playwright_screenshot: Ekran görüntüsü kaydet',
      'playwright_click: Buton veya bağlantıya tıkla',
    ],
  ),
  GlobalMcpServer(
    id: 'firecrawl',
    name: 'Firecrawl',
    category: 'Web & Search',
    description:
        'Dinamik JavaScript web sitelerini ve dokümantasyon ağaçlarını temiz LLM Markdown olarak kazıma.',
    command: 'npx',
    args: ['-y', 'firecrawl-mcp'],
    requiredEnvKeys: ['FIRECRAWL_API_KEY'],
    discoveredTools: [
      'scrape_url: Sayfayı temiz metin olarak kazı',
      'crawl_domain: Tüm siteyi haritalandırıp tara',
    ],
  ),

  // 4. Cloud & DevOps
  GlobalMcpServer(
    id: 'docker',
    name: 'Docker',
    category: 'DevOps & Cloud',
    description:
        'Yerel Docker daemon bağlantısıyla konteynerleri listeleme, log okuma, başlatma ve durdurma.',
    command: 'uvx',
    args: ['mcp-server-docker'],
    requiredEnvKeys: [],
    discoveredTools: [
      'list_containers: Çalışan ve durdurulan konteynerleri göster',
      'get_container_logs: Konteyner loglarını oku',
      'restart_container: Konteyneri yeniden başlat',
    ],
  ),
  GlobalMcpServer(
    id: 'cloudflare',
    name: 'Cloudflare',
    category: 'DevOps & Cloud',
    description:
        'Cloudflare Workers, KV, R2 nesne depolama ve D1 veritabanı kaynaklarını kontrol etme.',
    command: 'npx',
    args: ['-y', '@cloudflare/mcp-server-cloudflare'],
    requiredEnvKeys: ['CLOUDFLARE_API_TOKEN'],
    discoveredTools: [
      'list_workers: Workers betiklerini listele',
      'query_d1: D1 veritabanı sorgula',
    ],
  ),
  GlobalMcpServer(
    id: 'kubernetes',
    name: 'Kubernetes',
    category: 'DevOps & Cloud',
    description:
        'Kubeconfig üzerinden küme podlarını, servislerini, dağıtımlarını okuma ve log kontrolü.',
    command: 'uvx',
    args: ['mcp-server-kubernetes'],
    requiredEnvKeys: [],
    discoveredTools: [
      'list_pods: Pod durumlarını listele',
      'get_pod_logs: Pod loglarını getir',
      'describe_resource: Kaynak detaylarını incele',
    ],
  ),

  // 5. Productivity & Workspace
  GlobalMcpServer(
    id: 'google_drive',
    name: 'Google Drive',
    category: 'Productivity',
    description:
        'Google Drive dosyalarını arama, Google Docs ve Sheets belgelerini okuyup analiz etme.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-gdrive'],
    requiredEnvKeys: ['GOOGLE_APPLICATION_CREDENTIALS'],
    discoveredTools: [
      'search_files: Drive dosyalarında arama yap',
      'read_file: Belge içeriğini oku',
    ],
  ),
  GlobalMcpServer(
    id: 'notion',
    name: 'Notion',
    category: 'Productivity',
    description:
        'Notion çalışma alanındaki sayfaları, veritabanlarını arama, okuma ve yeni blok ekleme.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-notion'],
    requiredEnvKeys: ['NOTION_API_KEY'],
    discoveredTools: [
      'search_pages: Sayfa ve veritabanı ara',
      'get_page_content: Sayfa bloklarını oku',
      'append_block: Sayfaya yeni içerik ekle',
    ],
  ),
  GlobalMcpServer(
    id: 'slack',
    name: 'Slack',
    category: 'Productivity',
    description:
        'Slack kanallarını listeleme, konuşma geçmişini getirme ve kanallara mesaj gönderme.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-slack'],
    requiredEnvKeys: ['SLACK_BOT_TOKEN'],
    discoveredTools: [
      'list_channels: Kanalları listele',
      'get_channel_history: Mesaj geçmişini oku',
      'post_message: Kanala mesaj gönder',
    ],
  ),
  GlobalMcpServer(
    id: 'discord',
    name: 'Discord',
    category: 'Productivity',
    description:
        'Discord sunucularında metin kanallarını izleme, mesajlaşma ve bildirim iletme.',
    command: 'npx',
    args: ['-y', 'mcp-server-discord'],
    requiredEnvKeys: ['DISCORD_BOT_TOKEN'],
    discoveredTools: [
      'list_guild_channels: Sunucu kanallarını listele',
      'send_message: Kanala mesaj gönder',
    ],
  ),
  GlobalMcpServer(
    id: 'obsidian',
    name: 'Obsidian',
    category: 'Productivity',
    description:
        'Yerel Obsidian kasanızdaki Markdown notlarını arama, okuma ve bağlantıları çözme.',
    command: 'npx',
    args: ['-y', 'mcp-obsidian'],
    requiredEnvKeys: ['OBSIDIAN_VAULT_PATH'],
    discoveredTools: [
      'search_notes: Notlarda arama yap',
      'read_note: Not içeriğini oku',
    ],
  ),
  GlobalMcpServer(
    id: 'filesystem',
    name: 'Filesystem',
    category: 'Productivity',
    description:
        'Belirlenen güvenli kök dizinler altında dosya okuma, yazma ve dizin ağacı çıkarma.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem'],
    requiredEnvKeys: ['MCP_ALLOWED_DIRECTORY'],
    discoveredTools: [
      'read_file: Dosya oku',
      'write_file: Dosyaya yaz',
      'list_directory: Dizin listele',
    ],
  ),

  // 6. Memory & Reasoning
  GlobalMcpServer(
    id: 'memory',
    name: 'Memory Graph',
    category: 'Memory & Reasoning',
    description:
        'Bilgi grafı (knowledge graph) tabanlı kalıcı ve yapılandırılmış uzun süreli bellek.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-memory'],
    requiredEnvKeys: [],
    discoveredTools: [
      'create_entities: Varlıkları hafızaya kaydet',
      'create_relations: Varlıklar arası ilişki kur',
      'read_graph: Bilgi grafını sorgula',
    ],
  ),
  GlobalMcpServer(
    id: 'sequential_thinking',
    name: 'Sequential Thinking',
    category: 'Memory & Reasoning',
    description:
        'Karmaşık problemleri alt adımlara bölen ve hipotezleri dinamik güncelleyen akıl yürütme.',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-sequential-thinking'],
    requiredEnvKeys: [],
    discoveredTools: [
      'sequential_thinking: Çok adımlı mantıksal düşünce süreci yürüt',
    ],
  ),
  GlobalMcpServer(
    id: 'time',
    name: 'Time & Timezone',
    category: 'Memory & Reasoning',
    description:
        'Yerel saat, UTC ve dünya saat dilimleri arasında dönüşüm ve tarih hesaplamaları.',
    command: 'uvx',
    args: ['mcp-server-time'],
    requiredEnvKeys: [],
    discoveredTools: [
      'get_current_time: Geçerli zamanı al',
      'convert_time: Saat dilimleri arasında dönüşüm yap',
    ],
  ),

  // 7. Finance
  GlobalMcpServer(
    id: 'stripe',
    name: 'Stripe',
    category: 'Finance',
    description:
        'Müşteri sorgulama, abonelik durumları, faturalar ve ödeme olaylarını inceleme.',
    command: 'npx',
    args: ['-y', 'stripe-mcp-server'],
    requiredEnvKeys: ['STRIPE_SECRET_KEY'],
    discoveredTools: [
      'search_customers: Müşteri ara',
      'get_subscription: Abonelik detaylarını oku',
      'list_invoices: Faturaları listele',
    ],
  ),
];
