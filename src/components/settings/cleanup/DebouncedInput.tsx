import React, { useEffect, useRef, useState } from "react";
import { Input } from "../../ui/Input";
import { Textarea } from "../../ui/Textarea";

interface DebouncedInputProps
  extends Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    "value" | "onChange"
  > {
  value: string;
  onCommit: (value: string) => void;
  variant?: "default" | "compact";
  delay?: number;
}

/** Text input that keeps a local draft and commits after a pause or on blur. */
export const DebouncedInput: React.FC<DebouncedInputProps> = ({
  value,
  onCommit,
  delay = 400,
  onBlur,
  ...props
}) => {
  const [draft, setDraft] = useState(value);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastCommitted = useRef(value);

  useEffect(() => {
    setDraft(value);
    lastCommitted.current = value;
  }, [value]);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const commit = (next: string) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    if (next !== lastCommitted.current) {
      lastCommitted.current = next;
      onCommit(next);
    }
  };

  return (
    <Input
      {...props}
      value={draft}
      onChange={(e) => {
        const next = e.target.value;
        setDraft(next);
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => commit(next), delay);
      }}
      onBlur={(e) => {
        commit(draft);
        onBlur?.(e);
      }}
    />
  );
};

interface NumberFieldProps {
  value: number;
  min: number;
  max: number;
  onCommit: (value: number) => void;
  errorMessage: string;
  ariaLabel: string;
  delay?: number;
}

/** Integer input that only commits values inside [min, max]. */
export const NumberField: React.FC<NumberFieldProps> = ({
  value,
  min,
  max,
  onCommit,
  errorMessage,
  ariaLabel,
  delay = 400,
}) => {
  const [draft, setDraft] = useState(String(value));
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const parse = (text: string): number | null => {
    if (!/^\d+$/.test(text.trim())) return null;
    const n = Number(text);
    return n >= min && n <= max ? n : null;
  };

  const commit = (text: string) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const n = parse(text);
    if (n !== null && n !== value) onCommit(n);
  };

  const invalid = parse(draft) === null;

  return (
    <div className="flex flex-col items-end gap-1">
      <Input
        type="text"
        inputMode="numeric"
        aria-label={ariaLabel}
        aria-invalid={invalid}
        variant="compact"
        className={`w-28 ${invalid ? "border-red-500" : ""}`}
        value={draft}
        onChange={(e) => {
          const next = e.target.value;
          setDraft(next);
          if (timer.current) clearTimeout(timer.current);
          timer.current = setTimeout(() => commit(next), delay);
        }}
        onBlur={() => {
          commit(draft);
          if (parse(draft) === null) setDraft(String(value));
        }}
      />
      {invalid && <p className="text-xs text-red-500">{errorMessage}</p>}
    </div>
  );
};

interface DebouncedTextareaProps
  extends Omit<
    React.TextareaHTMLAttributes<HTMLTextAreaElement>,
    "value" | "onChange"
  > {
  value: string;
  onCommit: (value: string) => void;
  variant?: "default" | "compact";
  delay?: number;
}

/** Textarea that keeps a local draft and commits after a pause or on blur. */
export const DebouncedTextarea: React.FC<DebouncedTextareaProps> = ({
  value,
  onCommit,
  delay = 400,
  onBlur,
  ...props
}) => {
  const [draft, setDraft] = useState(value);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastCommitted = useRef(value);

  useEffect(() => {
    setDraft(value);
    lastCommitted.current = value;
  }, [value]);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const commit = (next: string) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    if (next !== lastCommitted.current) {
      lastCommitted.current = next;
      onCommit(next);
    }
  };

  return (
    <Textarea
      {...props}
      value={draft}
      onChange={(e) => {
        const next = e.target.value;
        setDraft(next);
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => commit(next), delay);
      }}
      onBlur={(e) => {
        commit(draft);
        onBlur?.(e);
      }}
    />
  );
};
