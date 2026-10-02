import React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../lib/utils';

const badgeVariants = cva(
  'inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium font-mono transition-all duration-150',
  {
    variants: {
      variant: {
        default: 'bg-zinc-800/80 text-zinc-200 border border-white/10 shadow-sm',
        outline: 'border border-white/15 text-zinc-300 bg-transparent',
        success: 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/25',
        groq: 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold',
        openrouter: 'bg-indigo-500/15 text-indigo-300 border border-indigo-500/30 font-semibold',
        violet: 'bg-purple-500/15 text-purple-300 border border-purple-500/30',
        warning: 'bg-amber-500/15 text-amber-300 border border-amber-500/30',
        destructive: 'bg-rose-500/15 text-rose-300 border border-rose-500/30',
        subtle: 'bg-white/5 text-zinc-400 border border-white/5',
      },
      size: {
        sm: 'px-2 py-0.5 text-[11px]',
        md: 'px-2.5 py-1 text-xs',
        lg: 'px-3 py-1.5 text-sm',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'sm',
    },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

export const Badge: React.FC<BadgeProps> = ({ className, variant, size, ...props }) => {
  return <div className={cn(badgeVariants({ variant, size }), className)} {...props} />;
};
