import { LoaderCircle } from 'lucide-react';
import { type ButtonHTMLAttributes, forwardRef, type ReactNode } from 'react';

import { cn } from '@/shared/utils/cn';

/** Semantic visual treatment available to shared buttons. */
export type ButtonVariant = 'primary' | 'secondary' | 'elevated' | 'ghost' | 'destructive';
/** Supported button density and shape presets. The small density keeps the 40px touch target. */
export type ButtonSize = 'default' | 'small' | 'icon';
export type ButtonShape = 'rounded' | 'pill';

/** Native button attributes with the design system's visual variants. */
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual intent; defaults to `secondary`. */
  variant?: ButtonVariant;
  /** Preset dimensions; defaults to `default`. */
  size?: ButtonSize;
  /** Corner treatment for controls such as compact filter chips. */
  shape?: ButtonShape;
  /** Announces a pending action and disables the button until it finishes. */
  loading?: boolean;
  /** Replaces the normal button content while loading. Use `null` for icon-only buttons. */
  loadingContent?: ReactNode;
}

const variantClasses: Record<ButtonVariant, string> = {
  primary: 'bg-accent text-accent-foreground hover:bg-accent-hover active:bg-accent-active',
  secondary: 'border border-border bg-transparent text-text hover:bg-hover',
  elevated: 'bg-selected text-text hover:bg-hover',
  ghost: 'bg-transparent text-secondary-text hover:bg-hover hover:text-text',
  destructive: 'bg-destructive text-white hover:brightness-90',
};

const sizeClasses: Record<ButtonSize, string> = {
  default: 'min-h-10 px-4',
  small: 'min-h-10 px-3 text-sm',
  icon: 'size-10 p-0',
};

const shapeClasses: Record<ButtonShape, string> = {
  rounded: 'rounded-button',
  pill: 'rounded-full',
};

/** Renders a keyboard-accessible native button using shared OpenChat styles. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    'aria-busy': ariaBusy,
    className,
    disabled,
    loading = false,
    loadingContent,
    variant = 'secondary',
    size = 'default',
    shape = 'rounded',
    type = 'button',
    children,
    ...props
  },
  ref,
) {
  return (
    <button
      aria-busy={loading ? true : ariaBusy}
      className={cn(
        'openchat-button inline-flex items-center justify-center gap-2 font-medium transition-colors duration-quick focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-45',
        variantClasses[variant],
        sizeClasses[size],
        shapeClasses[shape],
        className,
      )}
      data-loading={loading || undefined}
      disabled={disabled || loading}
      ref={ref}
      type={type}
      {...props}
    >
      {loading ? (
        <>
          <LoaderCircle aria-hidden="true" className="shrink-0 animate-spin" size={16} />
          {loadingContent === undefined ? children : loadingContent}
        </>
      ) : (
        children
      )}
    </button>
  );
});
