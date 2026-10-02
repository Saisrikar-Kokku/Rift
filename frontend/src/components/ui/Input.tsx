import React, { forwardRef } from 'react';
import { clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  hint?: string;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(({
  label,
  error,
  hint,
  leftIcon,
  rightIcon,
  className,
  id,
  ...props
}, ref) => {
  const inputId = id || (label ? label.toLowerCase().replace(/\s+/g, '-') : undefined);

  return (
    <div className="w-full flex flex-col gap-1.5">
      {label && (
        <label htmlFor={inputId} className="text-xs font-semibold text-slate-300 tracking-wide flex items-center justify-between">
          <span>{label}</span>
          {hint && <span className="text-[11px] font-normal text-slate-500">{hint}</span>}
        </label>
      )}
      <div className="relative flex items-center">
        {leftIcon && (
          <div className="absolute left-3 text-slate-400 pointer-events-none flex items-center justify-center">
            {leftIcon}
          </div>
        )}
        <input
          ref={ref}
          id={inputId}
          className={twMerge(clsx(
            "w-full bg-[#141420] text-slate-100 text-sm rounded-lg px-3.5 py-2.5 outline-none border transition-all duration-150",
            "border-white/10 hover:border-white/20 focus:border-[#7c5cfa] focus:ring-1 focus:ring-[#7c5cfa]",
            leftIcon && "pl-10",
            rightIcon && "pr-10",
            error && "border-rose-500 focus:border-rose-500 focus:ring-rose-500/50",
            className
          ))}
          {...props}
        />
        {rightIcon && (
          <div className="absolute right-3 flex items-center justify-center">
            {rightIcon}
          </div>
        )}
      </div>
      {error && (
        <span className="text-xs text-rose-400 mt-0.5">{error}</span>
      )}
    </div>
  );
});

Input.displayName = 'Input';
