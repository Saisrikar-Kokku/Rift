import React from 'react';
import { cn } from '../../lib/utils';

export interface KbdProps extends React.HTMLAttributes<HTMLElement> {
  size?: 'sm' | 'md' | 'lg';
  active?: boolean;
}

export const Kbd: React.FC<KbdProps> = ({
  className,
  children,
  size = 'md',
  active = false,
  ...props
}) => {
  const sizeClasses = {
    sm: 'px-1.5 py-0.5 text-[10px] min-w-[20px] h-[20px]',
    md: 'px-2 py-1 text-xs min-w-[26px] h-[26px]',
    lg: 'px-3 py-1.5 text-sm min-w-[34px] h-[34px]',
  };

  return (
    <kbd
      className={cn(
        'inline-flex items-center justify-center font-mono font-semibold rounded-md select-none transition-all duration-100',
        'bg-[#181824] text-zinc-200 border border-white/10 shadow-[0_2px_0_0_rgba(255,255,255,0.08),0_3px_6px_rgba(0,0,0,0.5)]',
        active && 'translate-y-[2px] shadow-[0_0px_0_0_rgba(255,255,255,0.08)] bg-purple-600/30 text-purple-200 border-purple-400/40',
        sizeClasses[size],
        className
      )}
      {...props}
    >
      {children}
    </kbd>
  );
};
