import type { TFunction } from 'i18next';
import type { ReactNode } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { Button, Card } from '@shared/components';
import type { StatisticsBreakdown, StatisticsTrend } from './statisticsData';
import { formatCost, formatNumber, formatTrendDate } from './statisticsData';

export function MetricCard({
  detail,
  label,
  value,
}: {
  detail?: string;
  label: string;
  value: string;
}) {
  return (
    <div className="grid content-start gap-1 bg-raised p-4">
      <p className="text-sm text-secondary-text">{label}</p>
      <p className="text-2xl font-semibold tabular-nums">{value}</p>
      {detail ? <p className="text-xs text-secondary-text">{detail}</p> : null}
    </div>
  );
}

export function TrendChart({
  fromDate,
  language,
  monthly,
  onShiftRange,
  points,
  canShiftForward,
  toDate,
  t,
}: {
  fromDate: Date;
  language: string;
  monthly: boolean;
  onShiftRange: (direction: -1 | 1) => void;
  points: StatisticsTrend[];
  canShiftForward: boolean;
  toDate: Date;
  t: TFunction;
}) {
  if (points.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>;
  const chartPoints = points.filter(
    (point) => point.requests > 0 || point.inputTokens > 0 || point.outputTokens > 0,
  );
  if (chartPoints.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>;
  const maximum = Math.max(1, ...chartPoints.map((point) => point.inputTokens + point.outputTokens));
  const scaleValue = (value: number) => Math.log1p(value);
  const scaleMaximum = scaleValue(maximum);
  const width = 720;
  const height = 180;
  const left = 8;
  const right = width - 8;
  const top = 8;
  const baseline = height - 28;
  const plotHeight = baseline - top;
  const groupWidth = Math.min((right - left) / chartPoints.length, 112);
  const chartOffset = left + ((right - left) - groupWidth * chartPoints.length) / 2;
  const barWidth = Math.max(4, Math.min(28, groupWidth * 0.34));
  const labelStep = Math.max(1, Math.ceil(chartPoints.length / 5));
  const totalInput = chartPoints.reduce((sum, point) => sum + point.inputTokens, 0);
  const totalOutput = chartPoints.reduce((sum, point) => sum + point.outputTokens, 0);
  const tokenTotal = totalInput + totalOutput;
  const dateFormat = new Intl.DateTimeFormat(language, { day: 'numeric', month: 'short', year: 'numeric' });
  const dateRange = `${dateFormat.format(fromDate)} – ${dateFormat.format(toDate)}`;
  const trendDescription = chartPoints
    .map((point) => {
      const label = formatTrendDate(point.bucket, language, monthly);
      return `${label}: ${t('statisticsInputTokens')} ${formatNumber(point.inputTokens, language)}, ${t('statisticsOutputTokens')} ${formatNumber(point.outputTokens, language)}, ${formatNumber(point.requests, language)} ${t('statisticsRequests')}`;
    })
    .join('. ');
  return (
    <div className="grid gap-3">
      <div className="statistics-trend-chart-wrap">
        <div className="mb-2 flex items-center justify-between gap-3 text-sm">
          <span className="font-medium">{dateRange}</span>
          <div className="flex items-center gap-1">
            <Button aria-label={t('statisticsPreviousPeriod')} onClick={() => onShiftRange(-1)} size="icon" variant="ghost">
              <ChevronLeft aria-hidden="true" size={16} />
            </Button>
            <Button
              aria-label={t('statisticsNextPeriod')}
              disabled={!canShiftForward}
              onClick={() => onShiftRange(1)}
              size="icon"
              variant="ghost"
            >
              <ChevronRight aria-hidden="true" size={16} />
            </Button>
          </div>
        </div>
        <svg
          aria-label={`${t('statisticsUsageTrend')}: ${trendDescription}`}
          className="statistics-trend-chart"
          role="img"
          viewBox={`0 0 ${width} ${height}`}
        >
          <title>{t('statisticsUsageTrend')}</title>
          {[0.25, 0.5, 0.75, 1].map((fraction) => {
            const y = baseline - plotHeight * fraction;
            return <line className="stroke-border" key={fraction} opacity="0.28" x1={left} x2={right} y1={y} y2={y} />;
          })}
          {chartPoints.map((point, index) => {
            const total = point.inputTokens + point.outputTokens;
            const totalHeight = total === 0 ? 0 : (scaleValue(total) / scaleMaximum) * plotHeight;
            const inputHeight = total === 0 ? 0 : (point.inputTokens / total) * totalHeight;
            const outputHeight = totalHeight - inputHeight;
            const centerX = chartOffset + groupWidth * (index + 0.5);
            const dateLabel = formatTrendDate(point.bucket, language, monthly);
            const description = `${dateLabel}: ${t('statisticsInputTokens')} ${formatNumber(point.inputTokens, language)}, ${t('statisticsOutputTokens')} ${formatNumber(point.outputTokens, language)}`;
            return (
              <g key={point.bucket}>
                <rect fill="rgb(var(--info))" height={inputHeight} rx="2" width={barWidth} x={centerX - barWidth / 2} y={baseline - inputHeight}>
                  <title>{description}</title>
                </rect>
                <rect fill="rgb(var(--accent))" height={outputHeight} rx="2" width={barWidth} x={centerX - barWidth / 2} y={baseline - totalHeight}>
                  <title>{description}</title>
                </rect>
                {index % labelStep === 0 || index === chartPoints.length - 1 ? (
                  <text className="fill-secondary-text" dominantBaseline="hanging" fontSize="10" textAnchor="middle" x={centerX} y={baseline + 8}>
                    {dateLabel}
                  </text>
                ) : null}
              </g>
            );
          })}
        </svg>
      </div>
      <div className="statistics-trend-summary">
        <TrendSummaryItem
          color="bg-info"
          label={t('statisticsInputTokens')}
          percent={tokenTotal === 0 ? 0 : totalInput / tokenTotal}
          tokens={totalInput}
          language={language}
        />
        <TrendSummaryItem
          color="bg-accent"
          label={t('statisticsOutputTokens')}
          percent={tokenTotal === 0 ? 0 : totalOutput / tokenTotal}
          tokens={totalOutput}
          language={language}
        />
      </div>
      <ul className="sr-only">
        {chartPoints.map((point) => (
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

function TrendSummaryItem({
  color,
  label,
  language,
  percent,
  tokens,
}: {
  color: string;
  label: string;
  language: string;
  percent: number;
  tokens: number;
}) {
  return (
    <div className="statistics-trend-summary-item">
      <span aria-hidden="true" className={`statistics-trend-summary-marker ${color}`} />
      <div className="grid gap-0.5">
        <span className="text-sm">{label}</span>
        <span className="text-sm font-semibold tabular-nums">{new Intl.NumberFormat(language, { style: 'percent', maximumFractionDigits: 1 }).format(percent)}</span>
        <span className="text-xs text-secondary-text">{formatNumber(tokens, language)}</span>
      </div>
    </div>
  );
}

export function BreakdownCard({
  iconFor,
  labelFor,
  items,
  language,
  title,
  t,
}: {
  items: StatisticsBreakdown[];
  iconFor?: (key: string) => ReactNode;
  labelFor?: (key: string) => string;
  language: string;
  title: string;
  t: TFunction;
}) {
  const maxTokens = Math.max(0, ...items.map((item) => item.inputTokens + item.outputTokens));
  return (
    <Card className="app-window-settings-card overflow-hidden rounded-card border border-border bg-raised">
      <div className="border-b border-border bg-surface/50 px-4 py-3">
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
              icon={iconFor?.(item.key)}
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

export function BreakdownRow({
  icon,
  item,
  label,
  language,
  maxTokens,
  t,
}: {
  item: StatisticsBreakdown;
  label: string;
  icon?: ReactNode;
  language: string;
  maxTokens: number;
  t: TFunction;
}) {
  const tokens = item.inputTokens + item.outputTokens;
  const width = maxTokens === 0 ? 0 : Math.min(100, (tokens / maxTokens) * 100);
  const estimatedCost = item.estimatedCostRequests > 0 ? formatCost(item.estimatedCostUsd, language) : null;
  return (
    <li
      className="grid gap-2 px-4 py-3"
      aria-label={`${label}, ${formatNumber(tokens, language)} ${t('statisticsTotalTokens')}, ${formatNumber(item.requests, language)} ${t('statisticsRequests')}${estimatedCost ? `, ${t('statisticsEstimatedCost')} ${estimatedCost}` : ''}`}
    >
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
        <span className="flex min-w-0 items-center gap-2 break-all font-medium">{icon}{label}</span>
        <span className="shrink-0 text-xs tabular-nums text-secondary-text">{formatNumber(tokens, language)}</span>
      </div>
      <div
        aria-label={`${t('statisticsTotalTokens')}: ${formatNumber(tokens, language)}`}
        className="h-1 overflow-hidden rounded-full bg-surface"
        role="img"
      >
        <div className="h-full rounded-full bg-accent" style={{ width: `${width}%` }} />
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-secondary-text">
        <span>
          {t('statisticsRequests')}: {formatNumber(item.requests, language)}
        </span>
        <span>
          {t('statisticsInputTokens')}: {formatNumber(item.inputTokens, language)}
        </span>
        <span>
          {t('statisticsOutputTokens')}: {formatNumber(item.outputTokens, language)}
        </span>
        {estimatedCost ? <span>{t('statisticsEstimatedCost')}: {estimatedCost}</span> : null}
      </div>
    </li>
  );
}
