import type { TFunction } from 'i18next';
import {
  Activity,
  ChartNoAxesCombined,
  ChevronRight,
  CircleAlert,
  Coins,
  Download,
  ListFilter,
  LoaderCircle,
  type LucideIcon,
  MessagesSquare,
  RefreshCw,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, Input, SegmentedControl, Select } from '@/design-system';
import type { JsonObject } from '@/platform/local-service';
import { localServiceClient } from '@/platform/local-service';

interface StatisticsSummary {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  conversationCount: number;
  providerReportedCostUsd: number;
  modelsDevCatalogCostUsd: number;
}

interface StatisticsBreakdown {
  key: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  cachedInputTokens: number;
  cacheWriteTokens: number;
  providerReportedCostUsd: number;
  costReportedRequests: number;
  modelsDevCatalogCostUsd: number;
  modelsDevCatalogCostRequests: number;
}

interface StatisticsTrend {
  bucket: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
}

interface StatisticsConversation {
  conversationId: string;
  title: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  modelsDevCatalogCostUsd: number;
  modelsDevCatalogCostRequests: number;
  lastRequestAtUnixMs: number;
}

interface StatisticsRequest {
  eventKind: string;
  startedAtUnixMs: number;
  conversationTitle: string;
  providerId: string | null;
  modelId: string | null;
  reasoningEffort: string | null;
  operation: string;
  status: string;
  fastRequested: boolean | null;
  inputTokens: number | null;
  outputTokens: number | null;
  reasoningTokens: number | null;
  cachedInputTokens: number | null;
  cacheWriteTokens: number | null;
  providerReportedCostUsd: number | null;
  modelsDevCatalogCostUsd: number | null;
  usageSource: string;
  runId: string | null;
}

interface StatisticsSnapshot {
  summary: StatisticsSummary;
  providers: StatisticsBreakdown[];
  models: StatisticsBreakdown[];
  dailyTrend: StatisticsTrend[];
  monthlyTrend: StatisticsTrend[];
  conversations: StatisticsConversation[];
  requestDetailCount: number;
  requests: StatisticsRequest[];
  filterOptions: { providers: string[]; models: string[] };
}

type LoadState = 'loading' | 'loaded' | 'failed';

export interface StatisticsSettingsSectionProps {
  onOpenConversation(conversationId: string): void;
}

export function StatisticsSettingsSection({ onOpenConversation }: StatisticsSettingsSectionProps) {
  const { i18n, t } = useTranslation();
  const today = useMemo(() => dateInputValue(new Date()), []);
  const monthAgo = useMemo(() => {
    const date = new Date();
    date.setDate(date.getDate() - 29);
    return dateInputValue(date);
  }, []);
  const [fromDate, setFromDate] = useState(monthAgo);
  const [toDate, setToDate] = useState(today);
  const [providerId, setProviderId] = useState('');
  const [modelId, setModelId] = useState('');
  const [trendPeriod, setTrendPeriod] = useState<'daily' | 'monthly'>('daily');
  const [snapshot, setSnapshot] = useState<StatisticsSnapshot | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [isExporting, setIsExporting] = useState(false);
  const [exportFailed, setExportFailed] = useState(false);
  const [exported, setExported] = useState(false);
  const [reloadVersion, setReloadVersion] = useState(0);
  const conversationDateFormat = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
  const requestGeneration = useRef(0);
  const downloadTimer = useRef<number | null>(null);
  const downloadUrl = useRef<string | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: reloadVersion requests a new statistics snapshot for unchanged filters.
  useEffect(() => {
    const generation = ++requestGeneration.current;
    const from = parseDateInput(fromDate);
    const to = parseDateInput(toDate);
    if (!from || !to || from > to) {
      setSnapshot(null);
      setLoadState('failed');
      return () => {
        requestGeneration.current += 1;
      };
    }

    setLoadState('loading');
    const load = async () => {
      try {
        const result = await localServiceClient.call(
          'statistics.get',
          statisticsQuery({
            fromDate: from,
            toDate: to,
            providerId: providerId || null,
            modelId: modelId || null,
            offset: 0,
            limit: 1,
          }),
        );
        const loaded = parseStatistics(result);
        if (requestGeneration.current !== generation) return;
        setSnapshot(loaded);
        setLoadState('loaded');
      } catch {
        if (requestGeneration.current === generation) setLoadState('failed');
      }
    };
    void load();
    return () => {
      requestGeneration.current += 1;
    };
  }, [fromDate, toDate, providerId, modelId, reloadVersion]);

  useEffect(
    () => () => {
      requestGeneration.current += 1;
      if (downloadTimer.current !== null) window.clearTimeout(downloadTimer.current);
      if (downloadUrl.current !== null) URL.revokeObjectURL(downloadUrl.current);
    },
    [],
  );

  const exportCsv = async () => {
    if (!snapshot || isExporting) return;
    const from = parseDateInput(fromDate);
    const to = parseDateInput(toDate);
    if (!from || !to || from > to) return;
    setIsExporting(true);
    setExportFailed(false);
    setExported(false);
    try {
      const requestRows: StatisticsRequest[] = [];
      let total = snapshot.requestDetailCount;
      let offset = 0;
      const pageSize = 200;
      while (offset < total) {
        const page = parseStatistics(
          await localServiceClient.call(
            'statistics.get',
            statisticsQuery({
              fromDate: from,
              toDate: to,
              providerId: providerId || null,
              modelId: modelId || null,
              offset,
              limit: pageSize,
            }),
          ),
        );
        total = page.requestDetailCount;
        if (page.requests.length === 0) break;
        requestRows.push(...page.requests);
        offset += page.requests.length;
      }
      const csvText = createStatisticsCsv(requestRows, t, i18n.language);
      const url = URL.createObjectURL(new Blob([csvText], { type: 'text/csv;charset=utf-8' }));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'openchat-usage-statistics.csv';
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      downloadUrl.current = url;
      setExported(true);
      downloadTimer.current = window.setTimeout(() => {
        URL.revokeObjectURL(url);
        downloadUrl.current = null;
        downloadTimer.current = null;
      }, 1000);
    } catch {
      setExportFailed(true);
    } finally {
      setIsExporting(false);
    }
  };

  const clearFilters = () => {
    setProviderId('');
    setModelId('');
  };
  const activeFilterCount = Number(providerId !== '') + Number(modelId !== '');
  const dateRangeInvalid = !parseDateInput(fromDate) || !parseDateInput(toDate) || fromDate > toDate;

  return (
    <section aria-labelledby="settings-statistics-heading" className="grid gap-5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold leading-7" id="settings-statistics-heading">
            {t('statistics')}
          </h2>
          <p className="mt-1 max-w-3xl text-sm text-secondary-text">{t('statisticsDescription')}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <details className="statistics-filter-menu relative">
            <summary
              aria-label={t('statisticsFilters')}
              className="statistics-filter-summary flex min-h-10 cursor-pointer items-center gap-2 rounded-control border border-border bg-raised px-3 text-sm font-medium text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <ListFilter aria-hidden="true" size={16} />
              <span>
                {t('statisticsFilters')}
                {activeFilterCount > 0 ? ` · ${activeFilterCount}` : ''}
              </span>
            </summary>
            <div className="statistics-filter-popover absolute right-0 top-full z-20 mt-2 grid w-statistics-filter gap-4 rounded-control border border-border bg-raised p-4 shadow-lg">
              <fieldset className="grid gap-3 sm:grid-cols-2">
                <legend className="text-sm font-semibold">{t('statisticsDateRange')}</legend>
                <Input
                  aria-label={t('statisticsDateFrom')}
                  disabled={isExporting}
                  label={t('statisticsDateFrom')}
                  max={toDate}
                  onChange={(event) => setFromDate(event.target.value)}
                  type="date"
                  value={fromDate}
                />
                <Input
                  aria-label={t('statisticsDateTo')}
                  disabled={isExporting}
                  label={t('statisticsDateTo')}
                  max={today}
                  min={fromDate}
                  onChange={(event) => setToDate(event.target.value)}
                  type="date"
                  value={toDate}
                />
              </fieldset>
              {snapshot ? (
                <div className="grid gap-3 sm:grid-cols-2">
                  <Select
                    disabled={isExporting}
                    label={t('statisticsProvider')}
                    onChange={(event) => setProviderId(event.target.value)}
                    options={[
                      { value: '', label: t('statisticsAll') },
                      ...snapshot.filterOptions.providers.map((id) => ({ value: id, label: providerLabel(id, t) })),
                    ]}
                    value={providerId}
                  />
                  <Select
                    disabled={isExporting}
                    label={t('statisticsModel')}
                    onChange={(event) => setModelId(event.target.value)}
                    options={[
                      { value: '', label: t('statisticsAll') },
                      ...snapshot.filterOptions.models.map((id) => ({ value: id, label: id })),
                    ]}
                    value={modelId}
                  />
                </div>
              ) : null}
              {activeFilterCount > 0 ? (
                <Button
                  className="justify-self-start"
                  disabled={isExporting}
                  onClick={clearFilters}
                  size="small"
                  variant="ghost"
                >
                  {t('statisticsClearFilters')}
                </Button>
              ) : null}
            </div>
          </details>
          <Button
            aria-label={t('statisticsRetry')}
            disabled={isExporting || (loadState === 'loading' && snapshot === null)}
            onClick={() => setReloadVersion((version) => version + 1)}
            size="icon"
            variant="secondary"
          >
            {loadState === 'loading' ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <RefreshCw aria-hidden="true" size={16} />
            )}
          </Button>
          <Button
            disabled={
              isExporting ||
              snapshot === null ||
              snapshot.requestDetailCount === 0 ||
              loadState === 'loading' ||
              dateRangeInvalid
            }
            onClick={() => void exportCsv()}
            variant="secondary"
          >
            {isExporting ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <Download aria-hidden="true" size={16} />
            )}
            {t(isExporting ? 'statisticsExporting' : 'statisticsExportCsv')}
          </Button>
        </div>
      </div>

      {dateRangeInvalid ? (
        <p className="text-sm text-destructive" role="alert">
          {t('statisticsDateRangeInvalid')}
        </p>
      ) : null}
      {loadState === 'failed' && snapshot === null && !dateRangeInvalid ? (
        <Card className="grid justify-items-center gap-3 p-6 text-center" role="alert">
          <ChartNoAxesCombined aria-hidden="true" className="text-secondary-icon" size={36} />
          <p className="text-sm text-destructive">{t('statisticsLoadFailed')}</p>
          <Button
            onClick={() => {
              requestGeneration.current += 1;
              setReloadVersion((version) => version + 1);
            }}
            size="small"
            variant="secondary"
          >
            {t('statisticsRetry')}
          </Button>
        </Card>
      ) : null}
      {loadState === 'loading' && snapshot === null ? (
        <div
          aria-label={t('historyLoading')}
          aria-live="polite"
          className="flex min-h-24 items-center justify-center"
          role="status"
        >
          <LoaderCircle aria-hidden="true" className="animate-spin text-secondary-icon" size={22} />
        </div>
      ) : null}
      {snapshot ? (
        <>
          {loadState === 'loading' ? (
            <div aria-label={t('statisticsRetry')} className="h-0.5 overflow-hidden bg-border" role="status">
              <div className="h-full w-1/3 animate-pulse bg-accent" />
            </div>
          ) : null}
          {loadState === 'failed' && !dateRangeInvalid ? (
            <div
              className="flex flex-wrap items-center gap-3 rounded-control border border-destructive bg-destructive/10 p-3"
              role="alert"
            >
              <CircleAlert aria-hidden="true" className="shrink-0 text-destructive" size={17} />
              <p className="flex-1 text-sm text-destructive">{t('statisticsLoadFailed')}</p>
              <Button
                onClick={() => {
                  requestGeneration.current += 1;
                  setReloadVersion((version) => version + 1);
                }}
                size="small"
                variant="ghost"
              >
                {t('statisticsRetry')}
              </Button>
            </div>
          ) : null}
          <div className="grid gap-3 sm:grid-cols-3">
            <MetricCard
              icon={Coins}
              label={t('statisticsTotalTokens')}
              value={formatNumber(snapshot.summary.totalTokens, i18n.language)}
              detail={`${t('statisticsInputTokens')}: ${formatNumber(snapshot.summary.inputTokens, i18n.language)} · ${t('statisticsOutputTokens')}: ${formatNumber(snapshot.summary.outputTokens, i18n.language)}`}
            />
            <MetricCard
              icon={Activity}
              label={t('statisticsRequests')}
              value={formatNumber(snapshot.summary.totalRequests, i18n.language)}
              detail={`${t('statisticsSuccessfulRequests')}: ${formatNumber(snapshot.summary.successfulRequests, i18n.language)}`}
            />
            <MetricCard
              icon={MessagesSquare}
              label={t('statisticsConversations')}
              value={formatNumber(snapshot.summary.conversationCount, i18n.language)}
            />
          </div>

          <Card className="grid gap-4 p-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h3 className="font-semibold">{t('statisticsUsageTrend')}</h3>
              <SegmentedControl
                label={t('statisticsUsageTrend')}
                onValueChange={(value) => setTrendPeriod(value === 'monthly' ? 'monthly' : 'daily')}
                options={[
                  { value: 'daily', label: t('statisticsDaily') },
                  { value: 'monthly', label: t('statisticsMonthly') },
                ]}
                value={trendPeriod}
              />
            </div>
            <TrendChart
              language={i18n.language}
              monthly={trendPeriod === 'monthly'}
              points={trendPeriod === 'monthly' ? snapshot.monthlyTrend : snapshot.dailyTrend}
              t={t}
            />
          </Card>

          <div className="grid gap-4 xl:grid-cols-2">
            <BreakdownCard
              items={snapshot.providers}
              labelFor={(id) => providerLabel(id, t)}
              title={t('statisticsProviders')}
              language={i18n.language}
              t={t}
            />
            <BreakdownCard items={snapshot.models} title={t('statisticsModels')} language={i18n.language} t={t} />
          </div>

          <Card className="overflow-hidden p-0">
            <div className="border-b border-border px-4 py-3">
              <h3 className="font-semibold">{t('statisticsConversations')}</h3>
            </div>
            {snapshot.conversations.length === 0 ? (
              <p className="p-4 text-sm text-secondary-text">{t('statisticsNoConversationData')}</p>
            ) : (
              <div className="divide-y divide-border">
                {snapshot.conversations.map((conversation) => {
                  const totalTokens = conversation.inputTokens + conversation.outputTokens;
                  const requestCount = formatNumber(conversation.requests, i18n.language);
                  const tokensLabel = formatNumber(totalTokens, i18n.language);
                  const costLabel =
                    conversation.modelsDevCatalogCostRequests === 0
                      ? null
                      : `${t('statisticsModelsDevCatalogCost')}: ${formatCost(conversation.modelsDevCatalogCostUsd, i18n.language)}`;
                  const accessibleLabel = [
                    conversation.title,
                    `${tokensLabel} ${t('statisticsTotalTokens')}`,
                    `${requestCount} ${t('statisticsRequests')}`,
                    costLabel,
                    t('statisticsOpenConversation'),
                  ]
                    .filter((part): part is string => part !== null)
                    .join(', ');
                  return (
                    <button
                      aria-label={accessibleLabel}
                      className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus"
                      key={conversation.conversationId}
                      onClick={() => onOpenConversation(conversation.conversationId)}
                      type="button"
                    >
                      <span className="grid min-w-0 flex-1 gap-1">
                        <span className="truncate text-sm text-text" title={conversation.title}>
                          {conversation.title}
                        </span>
                        <span className="truncate text-xs text-secondary-text">
                          {conversationDateFormat.format(new Date(conversation.lastRequestAtUnixMs))} · {requestCount}{' '}
                          {t('statisticsRequests')}
                        </span>
                        {costLabel ? <span className="truncate text-xs text-secondary-text">{costLabel}</span> : null}
                      </span>
                      <span className="shrink-0 text-xs font-semibold tabular-nums text-text">{tokensLabel}</span>
                      <ChevronRight aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
                    </button>
                  );
                })}
              </div>
            )}
          </Card>
          {exportFailed ? (
            <p className="text-sm text-destructive" role="alert">
              {t('statisticsExportFailed')}
            </p>
          ) : null}
          {exported ? (
            <p aria-live="polite" className="text-sm text-success" role="status">
              {t('statisticsExported')}
            </p>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function MetricCard({
  detail,
  icon: Icon,
  label,
  value,
}: {
  detail?: string;
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <Card className="grid min-h-28 gap-2 p-4">
      <div className="flex items-center gap-2">
        <Icon aria-hidden="true" className="text-accent" size={16} />
        <p className="text-sm text-secondary-text">{label}</p>
      </div>
      <p className="text-2xl font-semibold tabular-nums">{value}</p>
      {detail ? <p className="text-xs text-secondary-text">{detail}</p> : null}
    </Card>
  );
}

function TrendChart({
  language,
  monthly,
  points,
  t,
}: {
  language: string;
  monthly: boolean;
  points: StatisticsTrend[];
  t: TFunction;
}) {
  const chartRef = useRef<SVGSVGElement | null>(null);
  const [chartWidth, setChartWidth] = useState(720);

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const nextWidth = Math.round(entry.contentRect.width);
      if (nextWidth > 0) setChartWidth((current) => (current === nextWidth ? current : nextWidth));
    });
    observer.observe(chart);
    return () => observer.disconnect();
  }, []);

  if (points.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>;
  const width = chartWidth;
  const height = 190;
  const left = 48;
  const right = 8;
  const top = 10;
  const bottom = 30;
  const plotWidth = width - left - right;
  const plotHeight = height - top - bottom;
  const maximum = Math.max(1, ...points.flatMap((point) => [point.inputTokens, point.outputTokens]));
  const xForIndex = (index: number) =>
    points.length === 1 ? left + plotWidth / 2 : left + (plotWidth * index) / (points.length - 1);
  const yForValue = (value: number) => top + plotHeight - (plotHeight * value) / maximum;
  const pathFor = (valueFor: (point: StatisticsTrend) => number) =>
    points
      .map((point, index) => {
        const command = index === 0 ? 'M' : 'L';
        return `${command} ${xForIndex(index).toFixed(2)} ${yForValue(valueFor(point)).toFixed(2)}`;
      })
      .join(' ');
  const inputPath = pathFor((point) => point.inputTokens);
  const outputPath = pathFor((point) => point.outputTokens);
  const labelStep = Math.max(1, Math.ceil(points.length / 6));
  const compactNumber = new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 });
  const trendDescription = points
    .map((point) => {
      const label = formatTrendDate(point.bucket, language, monthly);
      return `${label}: ${t('statisticsInputTokens')} ${formatNumber(point.inputTokens, language)}, ${t('statisticsOutputTokens')} ${formatNumber(point.outputTokens, language)}, ${formatNumber(point.requests, language)} ${t('statisticsRequests')}`;
    })
    .join('. ');
  return (
    <div className="grid gap-3">
      <svg
        aria-label={`${t('statisticsUsageTrend')}: ${trendDescription}`}
        className="h-usage-chart w-full overflow-visible"
        ref={chartRef}
        role="img"
        viewBox={`0 0 ${width} ${height}`}
      >
        <title>{t('statisticsUsageTrend')}</title>
        {Array.from({ length: 5 }, (_, tick) => {
          const fraction = tick / 4;
          const y = top + plotHeight - plotHeight * fraction;
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: Each tick has a fixed coordinate on this five-step chart axis.
            <g key={tick}>
              <line
                className="stroke-border"
                opacity="0.45"
                strokeWidth="1"
                x1={left}
                x2={width - right}
                y1={y}
                y2={y}
              />
              <text
                className="fill-secondary-text"
                dominantBaseline="middle"
                fontSize="10"
                textAnchor="end"
                x={left - 7}
                y={y}
              >
                {compactNumber.format(maximum * fraction)}
              </text>
            </g>
          );
        })}
        {points.length > 1 ? (
          <>
            <path
              d={inputPath}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
            />
            <path
              d={outputPath}
              fill="none"
              stroke="rgb(var(--info))"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
            />
          </>
        ) : null}
        {points.map((point, index) => (
          <g key={point.bucket}>
            <circle cx={xForIndex(index)} cy={yForValue(point.inputTokens)} fill="rgb(var(--accent))" r="2.5" />
            <circle cx={xForIndex(index)} cy={yForValue(point.outputTokens)} fill="rgb(var(--info))" r="2.5" />
            {index % labelStep === 0 || index === points.length - 1 ? (
              <text
                className="fill-secondary-text"
                dominantBaseline="hanging"
                fontSize="10"
                textAnchor="middle"
                x={xForIndex(index)}
                y={top + plotHeight + 8}
              >
                {formatTrendDate(point.bucket, language, monthly)}
              </text>
            ) : null}
          </g>
        ))}
      </svg>
      <div aria-hidden="true" className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-secondary-text">
        <span className="flex items-center gap-2">
          <span className="size-2 rounded-full bg-accent" />
          {t('statisticsInputTokens')}
        </span>
        <span className="flex items-center gap-2">
          <span className="size-2 rounded-full bg-info" />
          {t('statisticsOutputTokens')}
        </span>
      </div>
      <ul className="sr-only">
        {points.map((point) => (
          <li key={point.bucket}>
            {formatTrendDate(point.bucket, language, monthly)}: {t('statisticsInputTokens')}{' '}
            {formatNumber(point.inputTokens, language)}, {t('statisticsOutputTokens')}{' '}
            {formatNumber(point.outputTokens, language)}, {formatNumber(point.requests, language)}{' '}
            {t('statisticsRequests')}
          </li>
        ))}
      </ul>
    </div>
  );
}

function BreakdownCard({
  labelFor,
  items,
  language,
  title,
  t,
}: {
  items: StatisticsBreakdown[];
  labelFor?: (key: string) => string;
  language: string;
  title: string;
  t: TFunction;
}) {
  const maxTokens = Math.max(0, ...items.map((item) => item.inputTokens + item.outputTokens));
  return (
    <Card className="overflow-hidden p-0">
      <div className="border-b border-border px-4 py-3">
        <h3 className="font-semibold">{title}</h3>
      </div>
      {items.length === 0 ? (
        <p className="p-4 text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>
      ) : (
        <ul className="divide-y divide-border">
          {items.map((item) => (
            <BreakdownRow
              item={item}
              key={item.key}
              label={labelFor?.(item.key) ?? item.key}
              language={language}
              maxTokens={maxTokens}
              t={t}
            />
          ))}
        </ul>
      )}
    </Card>
  );
}

function BreakdownRow({
  item,
  label,
  language,
  maxTokens,
  t,
}: {
  item: StatisticsBreakdown;
  label: string;
  language: string;
  maxTokens: number;
  t: TFunction;
}) {
  const tokens = item.inputTokens + item.outputTokens;
  const width = maxTokens === 0 ? 0 : Math.min(100, (tokens / maxTokens) * 100);
  const costLabels = [
    item.costReportedRequests > 0
      ? `${t('statisticsProviderReportedCost')}: ${formatCost(item.providerReportedCostUsd, language)} · ${t('statisticsCostCoverage', { reported: formatNumber(item.costReportedRequests, language) })}`
      : null,
    item.modelsDevCatalogCostRequests > 0
      ? `${t('statisticsModelsDevCatalogCost')}: ${formatCost(item.modelsDevCatalogCostUsd, language)} · ${t('statisticsModelsDevCatalogCostCoverage', { priced: formatNumber(item.modelsDevCatalogCostRequests, language) })}`
      : null,
  ].filter((value): value is string => value !== null);
  return (
    <li
      className="grid gap-1 px-4 py-3"
      aria-label={`${label}, ${formatNumber(tokens, language)} ${t('statisticsTotalTokens')}, ${formatNumber(item.requests, language)} ${t('statisticsRequests')}${costLabels.length > 0 ? `, ${costLabels.join(', ')}` : ''}`}
    >
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
        <span className="min-w-0 break-all font-medium">{label}</span>
        <span className="shrink-0 text-xs tabular-nums text-secondary-text">{formatNumber(tokens, language)}</span>
      </div>
      <div
        aria-label={`${t('statisticsTotalTokens')}: ${formatNumber(tokens, language)}`}
        className="h-1 overflow-hidden rounded-full bg-raised"
        role="img"
      >
        <div className="h-full rounded-full bg-accent" style={{ width: `${width}%` }} />
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-secondary-text">
        <span>
          {formatNumber(item.requests, language)} {t('statisticsRequests')}
        </span>
        <span>
          {t('statisticsInputTokens')}: {formatNumber(item.inputTokens, language)}
        </span>
        <span>
          {t('statisticsOutputTokens')}: {formatNumber(item.outputTokens, language)}
        </span>
        {costLabels.map((costLabel) => (
          <span key={costLabel}>{costLabel}</span>
        ))}
      </div>
    </li>
  );
}

function statisticsQuery({
  fromDate,
  limit,
  modelId,
  offset,
  providerId,
  toDate,
}: {
  fromDate: Date;
  toDate: Date;
  providerId: string | null;
  modelId: string | null;
  offset: number;
  limit: number;
}): JsonObject {
  const fromUnixMs = fromDate.getTime();
  const end = new Date(toDate);
  end.setDate(end.getDate() + 1);
  const toUnixMs = Math.min(end.getTime(), Date.now() + 1);
  return {
    fromUnixMs,
    toUnixMs: Math.max(fromUnixMs + 1, toUnixMs),
    providerId,
    modelId,
    offset,
    limit,
  };
}

function parseStatistics(value: JsonObject): StatisticsSnapshot {
  const summary = objectValue(value.summary, 'summary');
  return {
    summary: {
      inputTokens: requiredInteger(summary, 'inputTokens'),
      outputTokens: requiredInteger(summary, 'outputTokens'),
      totalTokens: requiredInteger(summary, 'totalTokens'),
      totalRequests: requiredInteger(summary, 'totalRequests'),
      successfulRequests: requiredInteger(summary, 'successfulRequests'),
      failedRequests: requiredInteger(summary, 'failedRequests'),
      conversationCount: requiredInteger(summary, 'conversationCount'),
      providerReportedCostUsd: requiredNumber(summary, 'providerReportedCostUsd'),
      modelsDevCatalogCostUsd: requiredNumber(summary, 'modelsDevCatalogCostUsd'),
    },
    providers: parseBreakdowns(value.providerBreakdown, 'providerId'),
    models: parseBreakdowns(value.modelBreakdown, 'modelId'),
    dailyTrend: parseTrends(value.dailyTrend, 'day'),
    monthlyTrend: parseTrends(value.monthlyTrend, 'month'),
    conversations: parseConversations(value.conversationRanking),
    requestDetailCount: requiredInteger(value, 'requestDetailCount'),
    requests: parseRequests(value.requestDetails),
    filterOptions: parseFilterOptions(value.filterOptions),
  };
}

function parseBreakdowns(value: unknown, keyName: string): StatisticsBreakdown[] {
  if (!Array.isArray(value)) throw new Error('Statistics breakdown is invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'breakdown');
    return {
      key: requiredString(item, keyName),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
      reasoningTokens: requiredInteger(item, 'reasoningTokens'),
      cachedInputTokens: requiredInteger(item, 'cachedInputTokens'),
      cacheWriteTokens: requiredInteger(item, 'cacheWriteTokens'),
      providerReportedCostUsd: requiredNumber(item, 'providerReportedCostUsd'),
      costReportedRequests: requiredInteger(item, 'costReportedRequests'),
      modelsDevCatalogCostUsd: requiredNumber(item, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: requiredInteger(item, 'modelsDevCatalogCostRequests'),
    };
  });
}

function parseTrends(value: unknown, bucketKey: string): StatisticsTrend[] {
  if (!Array.isArray(value)) throw new Error('Statistics trend is invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'trend');
    return {
      bucket: requiredString(item, bucketKey),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
    };
  });
}

function parseConversations(value: unknown): StatisticsConversation[] {
  if (!Array.isArray(value)) throw new Error('Statistics conversations are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'conversation');
    const lastRequestAtUnixMs = requiredInteger(item, 'lastRequestAtUnixMs');
    if (lastRequestAtUnixMs < 0 || lastRequestAtUnixMs > 8_640_000_000_000_000) {
      throw new Error('Statistics conversation timestamp is invalid.');
    }
    return {
      conversationId: requiredString(item, 'conversationId'),
      title: requiredString(item, 'title'),
      requests: requiredInteger(item, 'requests'),
      inputTokens: requiredInteger(item, 'inputTokens'),
      outputTokens: requiredInteger(item, 'outputTokens'),
      modelsDevCatalogCostUsd: requiredNumber(item, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: requiredInteger(item, 'modelsDevCatalogCostRequests'),
      lastRequestAtUnixMs,
    };
  });
}

function parseRequests(value: unknown): StatisticsRequest[] {
  if (!Array.isArray(value)) throw new Error('Statistics request details are invalid.');
  return value.map((entry) => {
    const item = objectValue(entry, 'request');
    return {
      eventKind: requiredString(item, 'eventKind'),
      startedAtUnixMs: requiredInteger(item, 'startedAtUnixMs'),
      conversationTitle: requiredString(item, 'conversationTitle'),
      providerId: nullableString(item.providerId),
      modelId: nullableString(item.modelId),
      reasoningEffort: nullableString(item.reasoningEffort),
      operation: requiredString(item, 'operation'),
      status: requiredString(item, 'status'),
      fastRequested: nullableBoolean(item.fastRequested),
      inputTokens: nullableInteger(item.inputTokens),
      outputTokens: nullableInteger(item.outputTokens),
      reasoningTokens: nullableInteger(item.reasoningTokens),
      cachedInputTokens: nullableInteger(item.cachedInputTokens),
      cacheWriteTokens: nullableInteger(item.cacheWriteTokens),
      providerReportedCostUsd: nullableNumber(item.providerReportedCostUsd),
      modelsDevCatalogCostUsd: nullableNumber(item.modelsDevCatalogCostUsd),
      usageSource: requiredString(item, 'usageSource'),
      runId: nullableString(item.runId),
    };
  });
}

function parseFilterOptions(value: unknown): { providers: string[]; models: string[] } {
  const options = objectValue(value, 'filter options');
  if (
    !Array.isArray(options.providers) ||
    !options.providers.every(isString) ||
    !Array.isArray(options.models) ||
    !options.models.every(isString)
  ) {
    throw new Error('Statistics filters are invalid.');
  }
  return { providers: options.providers, models: options.models };
}

function createStatisticsCsv(requests: readonly StatisticsRequest[], t: TFunction, language: string): string {
  const rows = [
    [
      t('statisticsRequestTime'),
      t('statisticsConversationTitle'),
      t('statisticsProvider'),
      t('statisticsModel'),
      t('statisticsOperation'),
      t('statisticsReasoningEffort'),
      t('statisticsFastModeFilter'),
      t('statisticsStatus'),
      t('statisticsInputTokens'),
      t('statisticsOutputTokens'),
      t('statisticsReasoningTokens'),
      t('statisticsCachedInputTokens'),
      t('statisticsCacheWriteTokens'),
      t('statisticsProviderReportedCost'),
      t('statisticsModelsDevCatalogCost'),
      t('statisticsUsageSource'),
      t('statisticsRunId'),
    ],
  ];
  for (const request of requests) {
    rows.push([
      new Intl.DateTimeFormat(language, { dateStyle: 'short', timeStyle: 'medium' }).format(request.startedAtUnixMs),
      request.conversationTitle,
      request.providerId === null ? t('statisticsNotReported') : providerLabel(request.providerId, t),
      request.modelId ?? t('statisticsNotReported'),
      operationLabel(request.operation, t),
      request.reasoningEffort === null
        ? t('statisticsNotReported')
        : request.reasoningEffort === 'unspecified'
          ? t('statisticsUnspecified')
          : request.reasoningEffort,
      request.fastRequested === null
        ? t('statisticsNotReported')
        : request.fastRequested
          ? t('statisticsRequested')
          : t('statisticsNotRequested'),
      statusLabel(request, t),
      request.inputTokens === null ? '' : String(request.inputTokens),
      request.outputTokens === null ? '' : String(request.outputTokens),
      request.reasoningTokens === null ? '' : String(request.reasoningTokens),
      request.cachedInputTokens === null ? '' : String(request.cachedInputTokens),
      request.cacheWriteTokens === null ? '' : String(request.cacheWriteTokens),
      request.providerReportedCostUsd === null ? '' : request.providerReportedCostUsd.toFixed(9),
      request.modelsDevCatalogCostUsd === null ? '' : request.modelsDevCatalogCostUsd.toFixed(9),
      request.usageSource,
      request.runId ?? '',
    ]);
  }
  return `\uFEFF${rows.map((row) => row.map(csvField).join(',')).join('\r\n')}`;
}

function csvField(value: string): string {
  return `"${value.replaceAll('"', '""')}"`;
}

function providerLabel(id: string, t: TFunction): string {
  if (id === 'chatgpt') return t('statisticsChatGptOAuth');
  if (id === 'chatgpt_api') return t('statisticsChatGptApi');
  return id;
}

function operationLabel(operation: string, t: TFunction): string {
  if (operation === 'chat') return t('statisticsOperationChat');
  if (operation === 'tool_follow_up') return t('statisticsOperationToolFollowUp');
  if (operation === 'compaction') return t('statisticsOperationCompaction');
  if (operation === 'title_generation') return t('statisticsOperationTitleGeneration');
  if (operation === 'legacy') return t('statisticsOperationLegacy');
  return operation;
}

function statusLabel(request: StatisticsRequest, t: TFunction): string {
  if (request.eventKind === 'legacy_output') return t('statisticsStatusLegacy');
  if (request.status === 'completed') return t('statisticsStatusCompleted');
  if (request.status === 'failed') return t('statisticsStatusFailed');
  if (request.status === 'cancelled') return t('statisticsStatusCancelled');
  if (request.status === 'interrupted') return t('statisticsStatusInterrupted');
  if (request.status === 'pending') return t('statisticsStatusPending');
  return t('statisticsNotReported');
}

function formatNumber(value: number, language: string): string {
  return new Intl.NumberFormat(language).format(value);
}

function formatCost(value: number, language: string): string {
  return new Intl.NumberFormat(language, { style: 'currency', currency: 'USD', maximumFractionDigits: 4 }).format(
    value,
  );
}

function formatTrendDate(value: string, language: string, monthly: boolean): string {
  const match = /^(\d{4})-(\d{2})(?:-(\d{2}))?$/u.exec(value);
  if (!match) return value;
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3] ?? 1));
  if (
    date.getFullYear() !== Number(match[1]) ||
    date.getMonth() !== Number(match[2]) - 1 ||
    (match[3] !== undefined && date.getDate() !== Number(match[3]))
  )
    return value;
  return new Intl.DateTimeFormat(language, monthly ? { month: 'short' } : { month: 'numeric', day: 'numeric' }).format(
    date,
  );
}

function dateInputValue(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 10);
}

function parseDateInput(value: string): Date | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const [yearPart, monthPart, dayPart] = value.split('-');
  if (yearPart === undefined || monthPart === undefined || dayPart === undefined) return null;
  const year = Number(yearPart);
  const month = Number(monthPart);
  const day = Number(dayPart);
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : null;
}

function objectValue(value: unknown, field: string): Record<string, unknown> {
  if (isRecord(value)) return value;
  throw new Error(`The statistics ${field} value is invalid.`);
}

function requiredString(value: Record<string, unknown>, key: string): string {
  const result = value[key];
  if (typeof result !== 'string' || result.length === 0) throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

function requiredInteger(value: Record<string, unknown>, key: string): number {
  const result = value[key];
  if (typeof result !== 'number' || !Number.isSafeInteger(result))
    throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

function requiredNumber(value: Record<string, unknown>, key: string): number {
  const result = value[key];
  if (typeof result !== 'number' || !Number.isFinite(result))
    throw new Error(`The statistics field ${key} is invalid.`);
  return result;
}

function nullableInteger(value: unknown): number | null {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isSafeInteger(value))
    throw new Error('A statistics token count is invalid.');
  return value;
}

function nullableBoolean(value: unknown): boolean | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'boolean') throw new Error('A statistics boolean value is invalid.');
  return value;
}

function nullableNumber(value: unknown): number | null {
  if (value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error('A statistics cost value is invalid.');
  return value;
}

function nullableString(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string') throw new Error('A statistics value is invalid.');
  return value;
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
