import {
  Activity,
  ChartNoAxesCombined,
  ChevronRight,
  CircleAlert,
  Coins,
  Download,
  ListFilter,
  LoaderCircle,
  MessagesSquare,
  RefreshCw,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card, DatePicker, SegmentedControl, Select } from '@/design-system';
import { localServiceClient } from '@/platform/local-service';
import { BreakdownCard, MetricCard, TrendChart } from './StatisticsCharts';
import type { LoadState, StatisticsRequest, StatisticsSnapshot } from './statisticsData';
import {
  createStatisticsCsv,
  dateInputValue,
  formatCost,
  formatNumber,
  parseDateInput,
  parseStatistics,
  providerLabel,
  statisticsQuery,
} from './statisticsData';

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
            <div className="openchat-dropdown-surface statistics-filter-popover absolute right-0 top-full z-20 mt-2 grid w-statistics-filter gap-4 p-4">
              <fieldset className="grid gap-3 sm:grid-cols-2">
                <legend className="text-sm font-semibold">{t('statisticsDateRange')}</legend>
                <DatePicker
                  disabled={isExporting}
                  label={t('statisticsDateFrom')}
                  max={toDate}
                  onChange={setFromDate}
                  value={fromDate}
                />
                <DatePicker
                  disabled={isExporting}
                  label={t('statisticsDateTo')}
                  max={today}
                  min={fromDate}
                  onChange={setToDate}
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
          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <MetricCard
              icon={ChartNoAxesCombined}
              label={t('statisticsTotalTokens')}
              value={formatNumber(snapshot.summary.totalTokens, i18n.language)}
              detail={`${t('statisticsInputTokens')}: ${formatNumber(snapshot.summary.inputTokens, i18n.language)} · ${t('statisticsOutputTokens')}: ${formatNumber(snapshot.summary.outputTokens, i18n.language)}`}
            />
            <MetricCard
              icon={Coins}
              label={t('statisticsEstimatedCost')}
              value={
                snapshot.summary.estimatedCostRequests === 0
                  ? t('unavailableValue')
                  : formatCost(snapshot.summary.estimatedCostUsd, i18n.language)
              }
              detail={t('statisticsEstimatedCostCoverage', {
                priced: formatNumber(snapshot.summary.estimatedCostRequests, i18n.language),
                total: formatNumber(snapshot.summary.totalRequests, i18n.language),
              })}
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
                    conversation.estimatedCostRequests === 0
                      ? null
                      : `${t('statisticsEstimatedCost')}: ${formatCost(conversation.estimatedCostUsd, i18n.language)}`;
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
