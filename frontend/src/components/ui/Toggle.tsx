import React from 'react';
import { clsx } from 'clsx';

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  description?: string;
  disabled?: boolean;
}

export const Toggle: React.FC<ToggleProps> = ({
  checked,
  onChange,
  label,
  description,
  disabled = false,
}) => {
  return (
    <div
      className={clsx(
        "flex items-center justify-between py-2 cursor-pointer select-none",
        disabled && "opacity-50 pointer-events-none"
      )}
      onClick={() => !disabled && onChange(!checked)}
    >
      {(label || description) && (
        <div className="flex flex-col pr-4">
          {label && <span className="text-sm font-semibold text-slate-200">{label}</span>}
          {description && <span className="text-xs text-slate-400 mt-0.5">{description}</span>}
        </div>
      )}
      <div
        className={clsx(
          "w-11 h-6 flex items-center rounded-full p-1 duration-200 ease-in-out transition-colors shrink-0",
          checked ? "bg-[#7c5cfa]" : "bg-slate-700/60"
        )}
      >
        <div
          className={clsx(
            "bg-white w-4 h-4 rounded-full shadow-md transform duration-200 ease-in-out",
            checked ? "translate-x-5" : "translate-x-0"
          )}
        />
      </div>
    </div>
  );
};
