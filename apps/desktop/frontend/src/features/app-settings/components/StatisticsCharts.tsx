import type { TFunction } from 'i18next';
import type { CSSProperties } from 'react';
import type { ReactNode } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import { Activity, ArrowDownToLine, ArrowUpFromLine, CalendarDays, CircleDollarSign } from 'lucide-react';
import { useState } from 'react';
import { Card } from '@shared/components';
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
  language,
  points,
  t,
}: {
  language: string;
  points: StatisticsTrend[];
  t: TFunction;
}) {
  const [hoveredPoint, setHoveredPoint] = useState<HoveredChartPoint | null>(null);
  if (points.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>;
  const chartPoints = points.filter(
    (point) => point.requests > 0 || point.inputTokens > 0 || point.outputTokens > 0,
  );
  if (chartPoints.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsNoBreakdownData')}</p>;
  const width = 720;
  const height = 220;
  const left = 16;
  const right = width - 12;
  const top = 12;
  const baseline = height - 34;
  const plotHeight = baseline - top;
  const maximum = Math.max(1, ...chartPoints.map((point) => point.inputTokens + point.outputTokens));
  const scaleValue = (value: number) => Math.log1p(value);
  const scaleMaximum = scaleValue(maximum);
  const labelStep = Math.max(1, Math.ceil(chartPoints.length / 5));
  const totalInput = chartPoints.reduce((sum, point) => sum + point.inputTokens, 0);
  const totalOutput = chartPoints.reduce((sum, point) => sum + point.outputTokens, 0);
  const tokenTotal = totalInput + totalOutput;
  const xForIndex = (index: number) =>
    chartPoints.length === 1 ? (left + right) / 2 : left + ((right - left) * index) / (chartPoints.length - 1);
  const inputPoints = chartPoints.map((point, index) => ({
    x: xForIndex(index),
    y: baseline - (scaleValue(point.inputTokens) / scaleMaximum) * plotHeight,
  }));
  const totalPoints = chartPoints.map((point, index) => ({
    x: xForIndex(index),
    y: baseline - (scaleValue(point.inputTokens + point.outputTokens) / scaleMaximum) * plotHeight,
  }));
  const inputArea = createAreaPath(inputPoints, baseline);
  const outputArea = createStackedAreaPath(totalPoints, inputPoints);
  const inputLine = createLinePath(inputPoints);
  const totalLine = createLinePath(totalPoints);
  const hoveredData = hoveredPoint === null ? null : chartPoints[hoveredPoint.index] ?? null;
  const hoveredInputPoint = hoveredPoint === null ? null : inputPoints[hoveredPoint.index] ?? null;
  const hoveredTotalPoint = hoveredPoint === null ? null : totalPoints[hoveredPoint.index] ?? null;
  return (
    <div className="grid gap-3">
      <div className="statistics-trend-chart-wrap">
        <svg
          aria-label={t('statisticsUsageTrend')}
          aria-describedby={hoveredPoint === null ? undefined : 'statistics-usage-chart-tooltip'}
          className="statistics-trend-chart"
          onPointerLeave={() => setHoveredPoint(null)}
          onPointerMove={(event) => setHoveredPoint(getHoveredPoint(event, chartPoints.length, left, right, width))}
          role="img"
          viewBox={`0 0 ${width} ${height}`}
        >
          {[0.25, 0.5, 0.75, 1].map((fraction) => {
            const y = baseline - plotHeight * fraction;
            return <line className="stroke-border" key={fraction} opacity="0.28" x1={left} x2={right} y1={y} y2={y} />;
          })}
          <path d={inputArea} fill="rgb(var(--info))" fillOpacity="0.24" />
          <path d={outputArea} fill="rgb(var(--accent))" fillOpacity="0.72" />
          <path d={inputLine} fill="none" stroke="rgb(var(--info))" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.5" />
          <path d={totalLine} fill="none" stroke="rgb(var(--accent))" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.5" />
          {chartPoints.map((point, index) => {
            const dateLabel = formatTrendDate(point.bucket, language, false);
            const description = `${dateLabel}: ${t('statisticsInputTokens')} ${formatNumber(point.inputTokens, language)}, ${t('statisticsOutputTokens')} ${formatNumber(point.outputTokens, language)}`;
            return index % labelStep === 0 || index === chartPoints.length - 1 ? (
              <g key={point.bucket}>
                <circle cx={inputPoints[index]?.x} cy={inputPoints[index]?.y} fill="rgb(var(--info))" r="3.5">
                  <title>{description}</title>
                </circle>
                <circle cx={totalPoints[index]?.x} cy={totalPoints[index]?.y} fill="rgb(var(--accent))" r="3.5">
                  <title>{description}</title>
                </circle>
                <text className="fill-secondary-text" dominantBaseline="hanging" fontSize="10" textAnchor="middle" x={xForIndex(index)} y={baseline + 8}>
                  {dateLabel}
                </text>
              </g>
            ) : null;
          })}
          {hoveredPoint !== null && hoveredInputPoint !== null && hoveredTotalPoint !== null ? (
            <g>
              <line
                stroke="rgb(var(--secondary-text))"
                strokeDasharray="3 3"
                strokeOpacity="0.65"
                x1={xForIndex(hoveredPoint.index)}
                x2={xForIndex(hoveredPoint.index)}
                y1={top}
                y2={baseline}
              />
              <circle cx={hoveredInputPoint.x} cy={hoveredInputPoint.y} fill="rgb(var(--info))" r="5" />
              <circle cx={hoveredTotalPoint.x} cy={hoveredTotalPoint.y} fill="rgb(var(--accent))" r="5" />
            </g>
          ) : null}
          <rect fill="transparent" height={baseline - top} pointerEvents="all" width={right - left} x={left} y={top} />
        </svg>
        {hoveredPoint !== null && hoveredData !== null ? (
          <div
            className="statistics-chart-tooltip"
            id="statistics-usage-chart-tooltip"
            role="tooltip"
            style={getTooltipPosition(hoveredPoint)}
          >
            <p className="statistics-chart-tooltip-heading">
              <CalendarDays aria-hidden="true" size={14} />
              <span>{formatTrendDate(hoveredData.bucket, language, false)}</span>
            </p>
            <div className="statistics-chart-tooltip-row">
              <span className="statistics-chart-tooltip-label"><ArrowDownToLine aria-hidden="true" size={14} />{t('statisticsInputTokens')}</span>
              <strong>{formatNumber(hoveredData.inputTokens, language)}</strong>
            </div>
            <div className="statistics-chart-tooltip-row">
              <span className="statistics-chart-tooltip-label"><ArrowUpFromLine aria-hidden="true" size={14} />{t('statisticsOutputTokens')}</span>
              <strong>{formatNumber(hoveredData.outputTokens, language)}</strong>
            </div>
            <div className="statistics-chart-tooltip-row">
              <span className="statistics-chart-tooltip-label"><Activity aria-hidden="true" size={14} />{t('statisticsRequests')}</span>
              <strong>{formatNumber(hoveredData.requests, language)}</strong>
            </div>
          </div>
        ) : null}
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
            {formatTrendDate(point.bucket, language, false)}: {t('statisticsInputTokens')}{' '}
            {formatNumber(point.inputTokens, language)}, {t('statisticsOutputTokens')}{' '}
            {formatNumber(point.outputTokens, language)}, {formatNumber(point.requests, language)}{' '}
            {t('statisticsRequests')}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function PriceTrendChart({
  language,
  points,
  t,
}: {
  language: string;
  points: StatisticsTrend[];
  t: TFunction;
}) {
  const [hoveredPoint, setHoveredPoint] = useState<HoveredChartPoint | null>(null);
  if (!points.some((point) => point.estimatedCostUsd > 0)) {
    return <p className="text-sm text-secondary-text">{t('statisticsPriceTrendUnavailable')}</p>;
  }
  const chartPoints = points.filter(
    (point) => point.requests > 0 || point.inputTokens > 0 || point.outputTokens > 0,
  );
  if (chartPoints.length === 0) return <p className="text-sm text-secondary-text">{t('statisticsPriceTrendUnavailable')}</p>;

  const maximum = Math.max(...chartPoints.map((point) => point.estimatedCostUsd));
  const width = 720;
  const height = 220;
  const left = 68;
  const right = width - 12;
  const top = 12;
  const baseline = height - 34;
  const plotHeight = baseline - top;
  const labelStep = Math.max(1, Math.ceil(chartPoints.length / 5));
  const xForIndex = (index: number) =>
    chartPoints.length === 1 ? (left + right) / 2 : left + ((right - left) * index) / (chartPoints.length - 1);
  const costPoints = chartPoints.map((point, index) => ({
    x: xForIndex(index),
    y: baseline - (maximum === 0 ? 0 : (point.estimatedCostUsd / maximum) * plotHeight),
  }));
  const costArea = createAreaPath(costPoints, baseline);
  const costLine = createLinePath(costPoints);
  const hoveredData = hoveredPoint === null ? null : chartPoints[hoveredPoint.index] ?? null;
  const hoveredCostPoint = hoveredPoint === null ? null : costPoints[hoveredPoint.index] ?? null;
  return (
    <div className="grid gap-3">
      <div className="statistics-trend-chart-wrap">
        <svg
          aria-label={t('statisticsPriceTrend')}
          aria-describedby={hoveredPoint === null ? undefined : 'statistics-price-chart-tooltip'}
          className="statistics-trend-chart"
          onPointerLeave={() => setHoveredPoint(null)}
          onPointerMove={(event) => setHoveredPoint(getHoveredPoint(event, chartPoints.length, left, right, width))}
          role="img"
          viewBox={`0 0 ${width} ${height}`}
        >
          {[0, 0.25, 0.5, 0.75, 1].map((fraction) => {
            const y = baseline - plotHeight * fraction;
            return (
              <g key={fraction}>
                <line className="stroke-border" opacity="0.28" x1={left} x2={right} y1={y} y2={y} />
                <text className="fill-secondary-text" dominantBaseline="middle" fontSize="10" textAnchor="end" x={left - 8} y={y}>
                  {formatCost(maximum * fraction, language)}
                </text>
              </g>
            );
          })}
          <path d={costArea} fill="rgb(var(--accent))" fillOpacity="0.16" />
          <path d={costLine} fill="none" stroke="rgb(var(--accent))" strokeLinecap="round" strokeLinejoin="round" strokeWidth="3" />
          {chartPoints.map((point, index) => {
            const centerX = xForIndex(index);
            const dateLabel = formatTrendDate(point.bucket, language, false);
            return index % labelStep === 0 || index === chartPoints.length - 1 ? (
              <g key={point.bucket}>
                <circle cx={costPoints[index]?.x} cy={costPoints[index]?.y} fill="rgb(var(--accent))" r="4">
                  <title>{`${dateLabel}: ${formatCost(point.estimatedCostUsd, language)}`}</title>
                </circle>
                <text className="fill-secondary-text" dominantBaseline="hanging" fontSize="10" textAnchor="middle" x={centerX} y={baseline + 8}>
                  {dateLabel}
                </text>
              </g>
            ) : null;
          })}
          {hoveredPoint !== null && hoveredCostPoint !== null ? (
            <g>
              <line
                stroke="rgb(var(--secondary-text))"
                strokeDasharray="3 3"
                strokeOpacity="0.65"
                x1={xForIndex(hoveredPoint.index)}
                x2={xForIndex(hoveredPoint.index)}
                y1={top}
                y2={baseline}
              />
              <circle cx={hoveredCostPoint.x} cy={hoveredCostPoint.y} fill="rgb(var(--accent))" r="5" />
            </g>
          ) : null}
          <rect fill="transparent" height={baseline - top} pointerEvents="all" width={right - left} x={left} y={top} />
        </svg>
        {hoveredPoint !== null && hoveredData !== null ? (
          <div
            className="statistics-chart-tooltip"
            id="statistics-price-chart-tooltip"
            role="tooltip"
            style={getTooltipPosition(hoveredPoint)}
          >
            <p className="statistics-chart-tooltip-heading">
              <CalendarDays aria-hidden="true" size={14} />
              <span>{formatTrendDate(hoveredData.bucket, language, false)}</span>
            </p>
            <div className="statistics-chart-tooltip-row">
              <span className="statistics-chart-tooltip-label"><CircleDollarSign aria-hidden="true" size={14} />{t('statisticsEstimatedCost')}</span>
              <strong>{formatCost(hoveredData.estimatedCostUsd, language)}</strong>
            </div>
          </div>
        ) : null}
      </div>
      <ul className="sr-only">
        {chartPoints.map((point) => (
          <li key={point.bucket}>
            {formatTrendDate(point.bucket, language, false)}: {formatCost(point.estimatedCostUsd, language)}
          </li>
        ))}
      </ul>
    </div>
  );
}

function createLinePath(points: readonly { x: number; y: number }[]): string {
  return points.map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x} ${point.y}`).join(' ');
}

interface HoveredChartPoint {
  index: number;
  xPercent: number;
  yPercent: number;
  alignStart: boolean;
  alignEnd: boolean;
  placeBelow: boolean;
}

function getHoveredPoint(
  event: ReactPointerEvent<SVGSVGElement>,
  pointCount: number,
  left: number,
  right: number,
  viewBoxWidth: number,
): HoveredChartPoint {
  const bounds = event.currentTarget.getBoundingClientRect();
  if (bounds.width <= 0 || bounds.height <= 0) {
    return { index: 0, xPercent: 0, yPercent: 0, alignStart: true, alignEnd: false, placeBelow: true };
  }
  const x = event.clientX - bounds.left;
  const y = event.clientY - bounds.top;
  const edgeThreshold = Math.min(144, bounds.width * 0.35);
  const svgX = ((event.clientX - bounds.left) / bounds.width) * viewBoxWidth;
  const plotRatio = Math.min(1, Math.max(0, (svgX - left) / (right - left)));
  return {
    index: Math.round(plotRatio * Math.max(0, pointCount - 1)),
    xPercent: (x / bounds.width) * 100,
    yPercent: (y / bounds.height) * 100,
    alignStart: x < edgeThreshold,
    alignEnd: x > bounds.width - edgeThreshold,
    placeBelow: y < bounds.height * 0.42,
  };
}

function getTooltipPosition(point: HoveredChartPoint): CSSProperties {
  const left = point.alignEnd ? 'calc(100% - 0.75rem)' : point.alignStart ? '0.75rem' : `${point.xPercent}%`;
  const top = point.placeBelow ? '0.75rem' : `${point.yPercent}%`;
  const translateX = point.alignEnd ? '-100%' : point.alignStart ? '0' : '-50%';
  const translateY = point.placeBelow ? '0' : 'calc(-100% - 0.75rem)';
  return {
    left,
    top,
    transform: `translate(${translateX}, ${translateY})`,
  };
}

function createAreaPath(points: readonly { x: number; y: number }[], baseline: number): string {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return '';
  return `${createLinePath(points)} L ${last.x} ${baseline} L ${first.x} ${baseline} Z`;
}

function createStackedAreaPath(
  upper: readonly { x: number; y: number }[],
  lower: readonly { x: number; y: number }[],
): string {
  const upperPath = createLinePath(upper);
  const lowerPath = [...lower].reverse();
  if (upperPath === '' || lowerPath.length === 0) return '';
  return `${upperPath} ${lowerPath.map((point) => `L ${point.x} ${point.y}`).join(' ')} Z`;
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
