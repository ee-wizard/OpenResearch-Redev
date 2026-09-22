import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentProps,
} from "react";
import { motion, useAnimate, useReducedMotion } from "motion/react";
import { arc } from "motion";
import { cn } from "../ui/cn";

const MotionA = motion.create("a");

const useIsomorphicLayoutEffect =
  typeof window !== "undefined" ? useLayoutEffect : useEffect;

export type BounceSidebarItem =
  | string
  | { label: string; href?: string }
  | { label: string; heading: true };

export type BounceSidebarProps = Omit<ComponentProps<"ul">, "onChange"> & {
  items: BounceSidebarItem[];
  value?: number;
  defaultValue?: number;
  onChange?: (index: number) => void;
};

export function BounceSidebar({
  items,
  value,
  defaultValue = 0,
  onChange,
  className,
  ...props
}: BounceSidebarProps) {
  const [internalValue, setInternalValue] = useState(defaultValue);
  const activeIndex = value ?? internalValue;
  const reduced = useReducedMotion();

  const [dot, animate] = useAnimate<HTMLSpanElement>();
  const itemRefs = useRef<(HTMLLIElement | null)[]>([]);
  const prevY = useRef<number | null>(null);

  const [dotSize, setDotSize] = useState(6);
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const dpr = window.devicePixelRatio || 1;
    setDotSize(Math.round(6 * dpr) / dpr);
  }, []);

  useIsomorphicLayoutEffect(() => {
    let cancelled = false;
    const snap = () => {
      const el = itemRefs.current[activeIndex];
      if (cancelled || !el || !dot.current) return;
      const dpr = window.devicePixelRatio || 1;
      const size = Math.round(6 * dpr) / dpr;
      const toY =
        Math.round((el.offsetTop + el.offsetHeight / 2 - size / 2) * dpr) / dpr;
      animate(dot.current, { x: 0, y: toY }, { duration: 0 });
      prevY.current = toY;
      setReady(true);
    };

    snap();
    const raf = requestAnimationFrame(snap);
    document.fonts?.ready.then(snap);
    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
  }, [reduced]);

  useEffect(() => {
    if (reduced) return;
    const el = itemRefs.current[activeIndex];
    if (!el || !dot.current) return;

    const dpr = window.devicePixelRatio || 1;
    const toY =
      Math.round((el.offsetTop + el.offsetHeight / 2 - dotSize / 2) * dpr) /
      dpr;

    if (prevY.current === null) {
      animate(dot.current, { x: 0, y: toY }, { duration: 0 });
      prevY.current = toY;
      return;
    }

    const fromY = prevY.current;
    const delta = toY - fromY;
    prevY.current = toY;
    if (delta === 0) return;

    const distance = Math.abs(delta);
    const path = arc({
      strength: Math.min(0.8, 14 / distance),
      direction: delta > 0 ? "ccw" : "cw",
    });

    animate(
      dot.current,
      { x: 0, y: toY },
      { duration: 0.25, ease: "easeOut", path },
    );
  }, [activeIndex, animate, dot, dotSize, reduced]);

  const select = (index: number) => {
    if (value === undefined) setInternalValue(index);
    onChange?.(index);
  };

  return (
    <ul
      data-slot="bounce-sidebar"
      className={cn("relative flex flex-col gap-1 pl-6", className)}
      {...props}
    >
      <span
        ref={dot}
        aria-hidden
        className="absolute left-2 top-0 rounded-full bg-primary transition-opacity duration-150"
        style={{
          width: dotSize,
          height: dotSize,
          opacity: ready ? 1 : 0,
        }}
      />

      {items.map((item, index) => {
        const label = typeof item === "string" ? item : item.label;

        if (typeof item !== "string" && "heading" in item) {
          return (
            <li
              key={`${index}-${label}`}
              ref={(el) => {
                itemRefs.current[index] = el;
              }}
              role="presentation"
              data-slot="bounce-sidebar-heading"
              className="px-1 pb-1 pt-7 text-xs font-semibold uppercase tracking-[0.14em] text-primary first:pt-0"
            >
              {label}
            </li>
          );
        }

        const href = typeof item === "string" ? undefined : item.href;
        const isActive = index === activeIndex;
        const itemClassName = cn(
          "flex w-full cursor-pointer items-center rounded-xl p-1 text-left text-sm transition-colors duration-200",
          isActive ? "text-text" : "text-muted",
        );

        return (
          <li
            key={`${index}-${label}`}
            ref={(el) => {
              itemRefs.current[index] = el;
            }}
          >
            {href ? (
              <MotionA
                href={href}
                data-slot="bounce-sidebar-item"
                data-active={isActive}
                onClick={() => select(index)}
                className={itemClassName}
                whileHover={reduced ? undefined : { x: 4 }}
                whileTap={reduced ? undefined : { scale: 0.98 }}
                transition={{ type: "spring", stiffness: 360, damping: 20 }}
              >
                {label}
              </MotionA>
            ) : (
              <motion.button
                type="button"
                data-slot="bounce-sidebar-item"
                data-active={isActive}
                onClick={() => select(index)}
                className={itemClassName}
                whileHover={reduced ? undefined : { x: 4 }}
                whileTap={reduced ? undefined : { scale: 0.98 }}
                transition={{ type: "spring", stiffness: 360, damping: 20 }}
              >
                {label}
              </motion.button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
