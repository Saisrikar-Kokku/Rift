import React, { useEffect, useRef, useState } from 'react';
import { cn } from '../../lib/utils';

export interface NumberTickerProps {
  value: number;
  direction?: 'up' | 'down';
  className?: string;
  delay?: number;
  duration?: number;
  formatCommas?: boolean;
}

export const NumberTicker: React.FC<NumberTickerProps> = ({
  value,
  className,
  delay = 0,
  duration = 1000,
  formatCommas = true,
}) => {
  const [displayValue, setDisplayValue] = useState(0);
  const startTimestamp = useRef<number | null>(null);
  const startValue = useRef<number>(0);

  useEffect(() => {
    startValue.current = displayValue;
    startTimestamp.current = null;

    let animationFrameId: number;

    const timeoutId = setTimeout(() => {
      const step = (timestamp: number) => {
        if (!startTimestamp.current) startTimestamp.current = timestamp;
        const progress = Math.min((timestamp - startTimestamp.current) / duration, 1);
        // Ease out quart
        const easeOut = 1 - Math.pow(1 - progress, 4);
        const current = Math.round(startValue.current + (value - startValue.current) * easeOut);
        setDisplayValue(current);

        if (progress < 1) {
          animationFrameId = requestAnimationFrame(step);
        } else {
          setDisplayValue(value);
        }
      };

      animationFrameId = requestAnimationFrame(step);
    }, delay);

    return () => {
      clearTimeout(timeoutId);
      cancelAnimationFrame(animationFrameId);
    };
  }, [value, duration, delay]);

  return (
    <span className={cn('inline-block tabular-nums tracking-tight', className)}>
      {formatCommas ? displayValue.toLocaleString() : displayValue}
    </span>
  );
};
