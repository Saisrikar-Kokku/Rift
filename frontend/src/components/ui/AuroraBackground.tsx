import React from 'react';
import { cn } from '../../lib/utils';

export const AuroraBackground: React.FC<{
  children?: React.ReactNode;
  className?: string;
  intensity?: 'subtle' | 'vibrant';
}> = ({ children, className, intensity = 'subtle' }) => {
  return (
    <div className={cn('relative overflow-hidden w-full h-full', className)}>
      <div
        className={cn(
          'pointer-events-none absolute -inset-[10px] opacity-40 blur-3xl filter transition-opacity duration-1000',
          intensity === 'vibrant' ? 'opacity-70' : 'opacity-30'
        )}
        style={{
          background:
            'radial-gradient(ellipse 60% 50% at 50% -20%, rgba(124, 58, 237, 0.25), transparent 70%), ' +
            'radial-gradient(ellipse 50% 40% at 85% 30%, rgba(56, 189, 248, 0.15), transparent 70%), ' +
            'radial-gradient(ellipse 45% 45% at 15% 70%, rgba(167, 139, 250, 0.15), transparent 70%)',
        }}
      />
      <div className="relative z-10 w-full h-full">{children}</div>
    </div>
  );
};
