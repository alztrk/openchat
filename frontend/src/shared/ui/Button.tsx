import type { ButtonHTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

type ButtonVariant = 'primary' | 'secondary' | 'elevated' | 'ghost' | 'destructive';
type ButtonSize = 'default' | 'small' | 'icon';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
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

export function Button({
  className,
  variant = 'secondary',
  size = 'default',
  type = 'button',
  ...props
}: ButtonProps) {
  return (
    <button
      className={cn(
        'openchat-button inline-flex items-center justify-center gap-2 rounded-button font-medium transition-colors duration-quick focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-45',
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
      type={type}
      {...props}
    />
  );
}
