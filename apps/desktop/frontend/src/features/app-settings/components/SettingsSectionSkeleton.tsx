import { Skeleton } from '@shared/components';

export function SettingsSectionSkeleton({ label }: { label: string }) {
  return (
    <div aria-label={label} aria-live="polite" className="grid content-start gap-4" role="status">
      <div aria-hidden="true" className="grid gap-3">
        {[0, 1, 2].map((row) => (
          <div className="flex min-h-20 items-center justify-between gap-4 rounded-card border border-border bg-raised p-4" key={row}>
            <div className="grid min-w-0 flex-1 gap-2">
              <Skeleton className="h-4 w-36 max-w-full" />
              <Skeleton className="h-3 w-64 max-w-full" />
            </div>
            <Skeleton className="h-7 w-12 shrink-0 rounded-full" />
          </div>
        ))}
      </div>
      <div aria-hidden="true" className="grid gap-3 sm:grid-cols-2">
        {[0, 1].map((card) => (
          <div className="grid min-h-28 content-between gap-4 rounded-card border border-border bg-raised p-4" key={card}>
            <div className="grid gap-2">
              <Skeleton className="h-4 w-40 max-w-full" />
              <Skeleton className="h-3 w-full" />
            </div>
            <Skeleton className="h-9 w-32 max-w-full" />
          </div>
        ))}
      </div>
    </div>
  );
}
