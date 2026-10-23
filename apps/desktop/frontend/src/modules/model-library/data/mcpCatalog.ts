import type { TFunction } from 'i18next';
import type english from '@/localization/locales/en.json';

export type McpDescriptionKey = Extract<keyof typeof english, `mcpDescription${string}`>;
export type McpCategoryKey = Extract<keyof typeof english, `mcpCategory${string}`>;
export type McpToolDescriptionKey = Extract<keyof typeof english, `mcpToolDescription${string}`>;

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

export const mcpToolDescriptionKeys: Readonly<Record<string, Readonly<Record<string, McpToolDescriptionKey>>>> = {
  github: {
    create_or_update_file: 'mcpToolDescriptionGithubCreateOrUpdateFile',
    search_repositories: 'mcpToolDescriptionGithubSearchRepositories',
    create_issue: 'mcpToolDescriptionGithubCreateIssue',
    get_issue: 'mcpToolDescriptionGithubGetIssue',
    list_pull_requests: 'mcpToolDescriptionGithubListPullRequests',
    create_pull_request: 'mcpToolDescriptionGithubCreatePullRequest',
  },
  git: {
    git_status: 'mcpToolDescriptionGitStatus',
    git_diff_unstaged: 'mcpToolDescriptionGitDiffUnstaged',
    git_log: 'mcpToolDescriptionGitLog',
    git_create_branch: 'mcpToolDescriptionGitCreateBranch',
  },
  gitlab: {
    list_projects: 'mcpToolDescriptionGitlabListProjects',
    get_merge_request: 'mcpToolDescriptionGitlabGetMergeRequest',
    create_issue: 'mcpToolDescriptionGitlabCreateIssue',
  },
  sentry: {
    get_issue: 'mcpToolDescriptionSentryGetIssue',
    search_events: 'mcpToolDescriptionSentrySearchEvents',
  },
  linear: {
    list_issues: 'mcpToolDescriptionLinearListIssues',
    create_issue: 'mcpToolDescriptionLinearCreateIssue',
    update_issue: 'mcpToolDescriptionLinearUpdateIssue',
  },
  postgres: {
    query: 'mcpToolDescriptionPostgresQuery',
    describe_table: 'mcpToolDescriptionPostgresDescribeTable',
    list_tables: 'mcpToolDescriptionPostgresListTables',
  },
  sqlite: {
    read_query: 'mcpToolDescriptionSqliteReadQuery',
    list_tables: 'mcpToolDescriptionSqliteListTables',
    describe_table: 'mcpToolDescriptionSqliteDescribeTable',
  },
  supabase: {
    execute_sql: 'mcpToolDescriptionSupabaseExecuteSql',
    list_buckets: 'mcpToolDescriptionSupabaseListBuckets',
    get_project_status: 'mcpToolDescriptionSupabaseGetProjectStatus',
  },
  redis: {
    get_key: 'mcpToolDescriptionRedisGetKey',
    list_keys: 'mcpToolDescriptionRedisListKeys',
  },
  brave_search: {
    brave_web_search: 'mcpToolDescriptionBraveSearchWebSearch',
    brave_local_search: 'mcpToolDescriptionBraveSearchLocalSearch',
  },
  fetch: {
    fetch_url: 'mcpToolDescriptionFetchUrl',
  },
  puppeteer: {
    navigate: 'mcpToolDescriptionPuppeteerNavigate',
    click: 'mcpToolDescriptionPuppeteerClick',
    fill: 'mcpToolDescriptionPuppeteerFill',
    screenshot: 'mcpToolDescriptionPuppeteerScreenshot',
  },
  playwright: {
    playwright_navigate: 'mcpToolDescriptionPlaywrightNavigate',
    playwright_screenshot: 'mcpToolDescriptionPlaywrightScreenshot',
    playwright_click: 'mcpToolDescriptionPlaywrightClick',
  },
  firecrawl: {
    scrape_url: 'mcpToolDescriptionFirecrawlScrapeUrl',
    crawl_domain: 'mcpToolDescriptionFirecrawlCrawlDomain',
  },
  docker: {
    list_containers: 'mcpToolDescriptionDockerListContainers',
    get_container_logs: 'mcpToolDescriptionDockerGetContainerLogs',
    restart_container: 'mcpToolDescriptionDockerRestartContainer',
  },
  cloudflare: {
    list_workers: 'mcpToolDescriptionCloudflareListWorkers',
    query_d1: 'mcpToolDescriptionCloudflareQueryD1',
  },
  kubernetes: {
    list_pods: 'mcpToolDescriptionKubernetesListPods',
    get_pod_logs: 'mcpToolDescriptionKubernetesGetPodLogs',
    describe_resource: 'mcpToolDescriptionKubernetesDescribeResource',
  },
  google_drive: {
    search_files: 'mcpToolDescriptionGoogleDriveSearchFiles',
    read_file: 'mcpToolDescriptionGoogleDriveReadFile',
  },
  notion: {
    search_pages: 'mcpToolDescriptionNotionSearchPages',
    get_page_content: 'mcpToolDescriptionNotionGetPageContent',
    append_block: 'mcpToolDescriptionNotionAppendBlock',
  },
  slack: {
    list_channels: 'mcpToolDescriptionSlackListChannels',
    get_channel_history: 'mcpToolDescriptionSlackGetChannelHistory',
    post_message: 'mcpToolDescriptionSlackPostMessage',
  },
  discord: {
    list_guild_channels: 'mcpToolDescriptionDiscordListGuildChannels',
    send_message: 'mcpToolDescriptionDiscordSendMessage',
  },
  obsidian: {
    search_notes: 'mcpToolDescriptionObsidianSearchNotes',
    read_note: 'mcpToolDescriptionObsidianReadNote',
  },
  filesystem: {
    read_file: 'mcpToolDescriptionFilesystemReadFile',
    write_file: 'mcpToolDescriptionFilesystemWriteFile',
    list_directory: 'mcpToolDescriptionFilesystemListDirectory',
  },
  memory: {
    create_entities: 'mcpToolDescriptionMemoryCreateEntities',
    create_relations: 'mcpToolDescriptionMemoryCreateRelations',
    read_graph: 'mcpToolDescriptionMemoryReadGraph',
  },
  sequential_thinking: {
    sequential_thinking: 'mcpToolDescriptionSequentialThinking',
  },
  time: {
    get_current_time: 'mcpToolDescriptionTimeGetCurrentTime',
    convert_time: 'mcpToolDescriptionTimeConvertTime',
  },
  stripe: {
    search_customers: 'mcpToolDescriptionStripeSearchCustomers',
    get_subscription: 'mcpToolDescriptionStripeGetSubscription',
    list_invoices: 'mcpToolDescriptionStripeListInvoices',
  },
};

export function formatMcpToolLabel(serverId: string, rawTool: string, translate: TFunction): string {
  const tool = rawTool.trim();
  const separatorIndex = tool.indexOf(':');
  const toolId = (separatorIndex === -1 ? tool : tool.slice(0, separatorIndex)).trim();

  if (!toolId) {
    return tool;
  }

  const descriptionKey = mcpToolDescriptionKeys[serverId]?.[toolId];
  return descriptionKey ? `${toolId}: ${translate(descriptionKey, { defaultValue: toolId })}` : tool;
}

export const popularMcpCatalog: readonly CuratedMcpServer[] = [
  {
    id: 'github',
    name: 'GitHub',
    category: 'developer',
    descriptionKey: 'mcpDescriptionGithub',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    requiredEnvKeys: ['GITHUB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: [
      'create_or_update_file',
      'search_repositories',
      'create_issue',
      'get_issue',
      'list_pull_requests',
      'create_pull_request',
    ],
  },
  {
    id: 'git',
    name: 'Git',
    category: 'developer',
    descriptionKey: 'mcpDescriptionGit',
    command: 'uvx',
    args: ['mcp-server-git'],
    requiredEnvKeys: [],
    discoveredTools: ['git_status', 'git_diff_unstaged', 'git_log', 'git_create_branch'],
  },
  {
    id: 'gitlab',
    name: 'GitLab',
    category: 'developer',
    descriptionKey: 'mcpDescriptionGitlab',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-gitlab'],
    requiredEnvKeys: ['GITLAB_PERSONAL_ACCESS_TOKEN'],
    discoveredTools: ['list_projects', 'get_merge_request', 'create_issue'],
  },
  {
    id: 'sentry',
    name: 'Sentry',
    category: 'developer',
    descriptionKey: 'mcpDescriptionSentry',
    command: 'uvx',
    args: ['mcp-server-sentry'],
    requiredEnvKeys: ['SENTRY_AUTH_TOKEN'],
    discoveredTools: ['get_issue', 'search_events'],
  },
  {
    id: 'linear',
    name: 'Linear',
    category: 'developer',
    descriptionKey: 'mcpDescriptionLinear',
    command: 'npx',
    args: ['-y', 'linear-mcp'],
    requiredEnvKeys: ['LINEAR_API_KEY'],
    discoveredTools: ['list_issues', 'create_issue', 'update_issue'],
  },
  {
    id: 'postgres',
    name: 'PostgreSQL',
    category: 'database',
    descriptionKey: 'mcpDescriptionPostgres',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-postgres'],
    requiredEnvKeys: ['DATABASE_URL'],
    discoveredTools: ['query', 'describe_table', 'list_tables'],
  },
  {
    id: 'sqlite',
    name: 'SQLite',
    category: 'database',
    descriptionKey: 'mcpDescriptionSqlite',
    command: 'uvx',
    args: ['mcp-server-sqlite'],
    requiredEnvKeys: ['SQLITE_DB_PATH'],
    discoveredTools: ['read_query', 'list_tables', 'describe_table'],
  },
  {
    id: 'supabase',
    name: 'Supabase',
    category: 'database',
    descriptionKey: 'mcpDescriptionSupabase',
    command: 'npx',
    args: ['-y', '@supabase/mcp-server'],
    requiredEnvKeys: ['SUPABASE_ACCESS_TOKEN'],
    discoveredTools: ['execute_sql', 'list_buckets', 'get_project_status'],
  },
  {
    id: 'redis',
    name: 'Redis',
    category: 'database',
    descriptionKey: 'mcpDescriptionRedis',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-redis'],
    requiredEnvKeys: ['REDIS_URL'],
    discoveredTools: ['get_key', 'list_keys'],
  },
  {
    id: 'brave_search',
    name: 'Brave Search',
    category: 'webSearch',
    descriptionKey: 'mcpDescriptionBraveSearch',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-brave-search'],
    requiredEnvKeys: ['BRAVE_API_KEY'],
    discoveredTools: ['brave_web_search', 'brave_local_search'],
  },
  {
    id: 'fetch',
    name: 'Fetch',
    category: 'webSearch',
    descriptionKey: 'mcpDescriptionFetch',
    command: 'uvx',
    args: ['mcp-server-fetch'],
    requiredEnvKeys: [],
    discoveredTools: ['fetch_url'],
  },
  {
    id: 'puppeteer',
    name: 'Puppeteer',
    category: 'webSearch',
    descriptionKey: 'mcpDescriptionPuppeteer',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-puppeteer'],
    requiredEnvKeys: [],
    discoveredTools: ['navigate', 'click', 'fill', 'screenshot'],
  },
  {
    id: 'playwright',
    name: 'Playwright',
    category: 'webSearch',
    descriptionKey: 'mcpDescriptionPlaywright',
    command: 'npx',
    args: ['-y', '@executeautomation/playwright-mcp-server'],
    requiredEnvKeys: [],
    discoveredTools: ['playwright_navigate', 'playwright_screenshot', 'playwright_click'],
  },
  {
    id: 'firecrawl',
    name: 'Firecrawl',
    category: 'webSearch',
    descriptionKey: 'mcpDescriptionFirecrawl',
    command: 'npx',
    args: ['-y', 'firecrawl-mcp'],
    requiredEnvKeys: ['FIRECRAWL_API_KEY'],
    discoveredTools: ['scrape_url', 'crawl_domain'],
  },
  {
    id: 'docker',
    name: 'Docker',
    category: 'devOpsCloud',
    descriptionKey: 'mcpDescriptionDocker',
    command: 'uvx',
    args: ['mcp-server-docker'],
    requiredEnvKeys: [],
    discoveredTools: ['list_containers', 'get_container_logs', 'restart_container'],
  },
  {
    id: 'cloudflare',
    name: 'Cloudflare',
    category: 'devOpsCloud',
    descriptionKey: 'mcpDescriptionCloudflare',
    command: 'npx',
    args: ['-y', '@cloudflare/mcp-server-cloudflare'],
    requiredEnvKeys: ['CLOUDFLARE_API_TOKEN'],
    discoveredTools: ['list_workers', 'query_d1'],
  },
  {
    id: 'kubernetes',
    name: 'Kubernetes',
    category: 'devOpsCloud',
    descriptionKey: 'mcpDescriptionKubernetes',
    command: 'uvx',
    args: ['mcp-server-kubernetes'],
    requiredEnvKeys: [],
    discoveredTools: ['list_pods', 'get_pod_logs', 'describe_resource'],
  },
  {
    id: 'google_drive',
    name: 'Google Drive',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionGoogleDrive',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-gdrive'],
    requiredEnvKeys: ['GOOGLE_APPLICATION_CREDENTIALS'],
    discoveredTools: ['search_files', 'read_file'],
  },
  {
    id: 'notion',
    name: 'Notion',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionNotion',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-notion'],
    requiredEnvKeys: ['NOTION_API_KEY'],
    discoveredTools: ['search_pages', 'get_page_content', 'append_block'],
  },
  {
    id: 'slack',
    name: 'Slack',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionSlack',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-slack'],
    requiredEnvKeys: ['SLACK_BOT_TOKEN'],
    discoveredTools: ['list_channels', 'get_channel_history', 'post_message'],
  },
  {
    id: 'discord',
    name: 'Discord',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionDiscord',
    command: 'npx',
    args: ['-y', 'mcp-server-discord'],
    requiredEnvKeys: ['DISCORD_BOT_TOKEN'],
    discoveredTools: ['list_guild_channels', 'send_message'],
  },
  {
    id: 'obsidian',
    name: 'Obsidian',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionObsidian',
    command: 'npx',
    args: ['-y', 'mcp-obsidian'],
    requiredEnvKeys: ['OBSIDIAN_VAULT_PATH'],
    discoveredTools: ['search_notes', 'read_note'],
  },
  {
    id: 'filesystem',
    name: 'Filesystem',
    category: 'productivity',
    descriptionKey: 'mcpDescriptionFilesystem',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem'],
    requiredEnvKeys: ['MCP_ALLOWED_DIRECTORY'],
    discoveredTools: ['read_file', 'write_file', 'list_directory'],
  },
  {
    id: 'memory',
    name: 'Memory Graph',
    category: 'memoryReasoning',
    descriptionKey: 'mcpDescriptionMemory',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-memory'],
    requiredEnvKeys: [],
    discoveredTools: ['create_entities', 'create_relations', 'read_graph'],
  },
  {
    id: 'sequential_thinking',
    name: 'Sequential Thinking',
    category: 'memoryReasoning',
    descriptionKey: 'mcpDescriptionSequentialThinking',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-sequential-thinking'],
    requiredEnvKeys: [],
    discoveredTools: ['sequential_thinking'],
  },
  {
    id: 'time',
    name: 'Time & Timezone',
    category: 'memoryReasoning',
    descriptionKey: 'mcpDescriptionTime',
    command: 'uvx',
    args: ['mcp-server-time'],
    requiredEnvKeys: [],
    discoveredTools: ['get_current_time', 'convert_time'],
  },
  {
    id: 'stripe',
    name: 'Stripe',
    category: 'finance',
    descriptionKey: 'mcpDescriptionStripe',
    command: 'npx',
    args: ['-y', 'stripe-mcp-server'],
    requiredEnvKeys: ['STRIPE_SECRET_KEY'],
    discoveredTools: ['search_customers', 'get_subscription', 'list_invoices'],
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
