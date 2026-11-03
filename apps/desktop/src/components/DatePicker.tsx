import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CalendarDays, ChevronLeft, ChevronRight, X } from "lucide-react";
import { intlLocale, t } from "../lib/i18n";

interface DatePickerProps {
  label: string;
  value: string;
  min?: string;
  max?: string;
  onChange: (value: string) => void;
}

function parseDate(value?: string): Date | undefined {
  if (!value) return undefined;
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) return undefined;
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : undefined;
}

function dateValue(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function startOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), 1);
}

function addMonths(date: Date, amount: number): Date {
  return new Date(date.getFullYear(), date.getMonth() + amount, 1);
}

function clampDate(date: Date, min?: Date, max?: Date): Date {
  if (min && date < min) return min;
  if (max && date > max) return max;
  return date;
}

function calendarDates(month: Date): Date[] {
  const first = startOfMonth(month);
  const mondayOffset = (first.getDay() + 6) % 7;
  const gridStart = new Date(first.getFullYear(), first.getMonth(), 1 - mondayOffset);
  return Array.from({ length: 42 }, (_, index) => new Date(gridStart.getFullYear(), gridStart.getMonth(), gridStart.getDate() + index));
}

function sameDate(left?: Date, right?: Date): boolean {
  return Boolean(left && right && left.getFullYear() === right.getFullYear() && left.getMonth() === right.getMonth() && left.getDate() === right.getDate());
}

export function DatePicker({ label, value, min, max, onChange }: DatePickerProps) {
  const selected = parseDate(value);
  const minimum = parseDate(min);
  const maximum = parseDate(max);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const [open, setOpen] = useState(false);
  const [month, setMonth] = useState(() => startOfMonth(selected ?? clampDate(today, minimum, maximum)));
  const [position, setPosition] = useState({ left: 0, top: 0 });
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const labelId = useId();
  const dialogId = useId();
  const displayFormatter = new Intl.DateTimeFormat(intlLocale, { day: "numeric", month: "short", year: "numeric" });
  const monthFormatter = new Intl.DateTimeFormat(intlLocale, { month: "long", year: "numeric" });
  const dayFormatter = new Intl.DateTimeFormat(intlLocale, { day: "numeric", month: "long", year: "numeric" });
  const weekdayFormatter = new Intl.DateTimeFormat(intlLocale, { weekday: "narrow" });
  const dates = calendarDates(month);

  const positionPopover = () => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const width = 286;
    const height = 322;
    const gap = 10;
    let left: number;
    let top: number;

    if (rect.left >= width + gap + 12) {
      left = rect.left - width - gap;
      top = rect.top;
    } else {
      left = Math.min(Math.max(12, rect.left), window.innerWidth - width - 12);
      top = rect.bottom + gap;
      if (top + height > window.innerHeight - 12) top = rect.top - height - gap;
    }

    setPosition({
      left: Math.round(left),
      top: Math.round(Math.min(Math.max(12, top), window.innerHeight - height - 12))
    });
  };

  const showCalendar = () => {
    setMonth(startOfMonth(selected ?? clampDate(today, minimum, maximum)));
    positionPopover();
    setOpen(true);
  };

  const closeCalendar = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) requestAnimationFrame(() => triggerRef.current?.focus());
  };

  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) closeCalendar();
    };
    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeCalendar(true);
    };
    const reposition = () => positionPopover();
    document.addEventListener("mousedown", closeOutside);
    window.addEventListener("keydown", handleKeydown);
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      document.removeEventListener("mousedown", closeOutside);
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [open]);

  const monthAvailable = (candidate: Date) => {
    const monthStart = startOfMonth(candidate);
    const monthEnd = new Date(candidate.getFullYear(), candidate.getMonth() + 1, 0);
    return (!minimum || monthEnd >= minimum) && (!maximum || monthStart <= maximum);
  };

  const chooseDate = (date: Date) => {
    onChange(dateValue(date));
    closeCalendar(true);
  };

  const calendar = open && typeof document !== "undefined" ? createPortal(
    <div
      ref={popoverRef}
      id={dialogId}
      className="date-picker-popover"
      role="dialog"
      aria-modal="false"
      aria-labelledby={labelId}
      style={{ left: position.left, top: position.top }}
    >
      <div className="date-picker-popover__header">
        <strong>{monthFormatter.format(month)}</strong>
        <div>
          <button type="button" onClick={() => setMonth(addMonths(month, -1))} disabled={!monthAvailable(addMonths(month, -1))} aria-label={t("Previous month")}><ChevronLeft size={16} /></button>
          <button type="button" onClick={() => setMonth(addMonths(month, 1))} disabled={!monthAvailable(addMonths(month, 1))} aria-label={t("Next month")}><ChevronRight size={16} /></button>
        </div>
      </div>
      <div className="date-picker-popover__weekdays" aria-hidden="true">
        {Array.from({ length: 7 }, (_, index) => <span key={index}>{weekdayFormatter.format(new Date(2024, 0, index + 1))}</span>)}
      </div>
      <div className="date-picker-popover__days">
        {dates.map((date) => {
          const iso = dateValue(date);
          const outside = date.getMonth() !== month.getMonth();
          const disabled = Boolean((minimum && date < minimum) || (maximum && date > maximum));
          return (
            <button
              type="button"
              key={iso}
              className={`${outside ? "is-outside" : ""} ${sameDate(date, today) ? "is-today" : ""} ${sameDate(date, selected) ? "is-selected" : ""}`}
              disabled={disabled}
              onClick={() => chooseDate(date)}
              aria-label={dayFormatter.format(date)}
              aria-pressed={sameDate(date, selected)}
            >
              {date.getDate()}
            </button>
          );
        })}
      </div>
    </div>,
    document.body
  ) : null;

  return (
    <div className="filter-date-picker" ref={rootRef}>
      <span className="filter-date-picker__label" id={labelId}>{label}</span>
      <div className={`filter-date-control ${open ? "is-open" : ""}`}>
        <button
          type="button"
          ref={triggerRef}
          className={`filter-date-trigger ${value ? "" : "is-empty"}`}
          onClick={() => open ? closeCalendar() : showCalendar()}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-controls={open ? dialogId : undefined}
        >
          <CalendarDays size={16} aria-hidden="true" />
          <span>{selected ? displayFormatter.format(selected) : t("Choose date")}</span>
        </button>
        {value && <button type="button" className="filter-date-clear" onClick={() => onChange("")} aria-label={t("Clear date")} title={t("Clear date")}><X size={14} /></button>}
      </div>
      {calendar}
    </div>
  );
}
