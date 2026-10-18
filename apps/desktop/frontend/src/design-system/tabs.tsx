import * as TabsPrimitive from '@radix-ui/react-tabs';
import type * as React from 'react';

import { cn } from '@/shared/utils/cn';

/** Available visual treatments for the tab list. */
type TabsListVariant = 'default' | 'line' | 'rounded';

/** Provides the Radix tab state and keyboard-navigation context. */
function Tabs({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Root>) {
  return <TabsPrimitive.Root className={cn('openchat-tabs flex min-w-0 flex-col', className)} {...props} />;
}

/** Renders the tab triggers with a shared visual variant. */
function TabsList({
  className,
  variant = 'default',
  ...props
}: React.ComponentProps<typeof TabsPrimitive.List> & { variant?: TabsListVariant }) {
  return <TabsPrimitive.List className={cn('openchat-tabs-list', className)} data-variant={variant} {...props} />;
}

/** A keyboard-navigable tab button associated with a `TabsContent` value. */
function TabsTrigger({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  return <TabsPrimitive.Trigger className={cn('openchat-tabs-trigger', className)} {...props} />;
}

/** Panel displayed when its matching tab trigger is active. */
function TabsContent({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Content>) {
  return <TabsPrimitive.Content className={cn('openchat-tabs-content', className)} {...props} />;
}

export { Tabs, TabsContent, TabsList, TabsTrigger };
