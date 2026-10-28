import type { TFunction } from 'i18next';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Card } from '@/design-system';
import type { StatisticsBreakdown, StatisticsTrend } from './statisticsData';
import { formatCost, formatNumber, formatTrendDate, reportedCacheReadShare } from './statisticsData';

export function MetricCard({
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
    <Card className="grid min-h-28 gap-2" padding="default">
      <div className="flex items-center gap-2">
        <Icon aria-hidden="true" className="text-accent" size={16} />
        <p className="text-sm text-secondary-text">{label}</p>
      </div>
      <p className="text-2xl font-semibold tabular-nums">{value}</p>
      {detail ? <p className="text-xs text-secondary-text">{detail}</p> : null}
    </Card>
  );
}

export function TrendChart({
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

export function BreakdownCard({
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
    <Card className="overflow-hidden">
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

export function BreakdownRow({
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
  const costLabels =
    item.estimatedCostRequests > 0
      ? [
          `${t('statisticsEstimatedCost')}: ${formatCost(item.estimatedCostUsd, language)} · ${t('statisticsEstimatedCostCoverage', { priced: formatNumber(item.estimatedCostRequests, language), total: formatNumber(item.requests, language) })}`,
        ]
      : [];
  const cacheShare = reportedCacheReadShare(item);
  const cacheShareLabel =
    item.cacheShareRequests > 0
      ? t('statisticsReportedCacheReadShare', {
          share:
            cacheShare === null
              ? t('unavailableValue')
              : new Intl.NumberFormat(language, { style: 'percent', maximumFractionDigits: 1 }).format(cacheShare),
          cached: formatNumber(item.cacheShareCachedTokens, language),
          input: formatNumber(item.cacheShareInputTokens, language),
          covered: formatNumber(item.cacheShareRequests, language),
          total: formatNumber(item.requests, language),
        })
      : null;
  return (
    <li
      className="grid gap-1 px-4 py-3"
      aria-label={`${label}, ${formatNumber(tokens, language)} ${t('statisticsTotalTokens')}, ${formatNumber(item.requests, language)} ${t('statisticsRequests')}${costLabels.length > 0 ? `, ${costLabels.join(', ')}` : ''}${cacheShareLabel ? `, ${cacheShareLabel}` : ''}`}
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
        {cacheShareLabel ? <span>{cacheShareLabel}</span> : null}
        {costLabels.map((costLabel) => (
          <span key={costLabel}>{costLabel}</span>
        ))}
      </div>
    </li>
  );
}
