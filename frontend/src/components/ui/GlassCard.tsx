import React from 'react';
import { clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

interface GlassCardProps extends React.HTMLAttributes<HTMLDivElement> {
  hoverable?: boolean;
}

export const GlassCard: React.FC<GlassCardProps> = ({
  children,
  className,
  hoverable = false,
  ...props
}) => {
  return (
    <div
      className={twMerge(clsx(
        "bg-[#12121e]/90 border border-white/[0.08] rounded-xl p-5 shadow-obsidian-inset",
        hoverable && "transition-all duration-200 hover:bg-[#161625] hover:border-white/[0.14] hover:shadow-glass-hover",
        className
      ))}
      {...props}
    >
      {children}
    </div>
  );
};
