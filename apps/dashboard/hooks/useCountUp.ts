import { useEffect, useRef, useState } from "react";

/**
 * useCountUp -- animate a number from 0 to target over duration ms.
 * Returns the current animated value as a string (formatted).
 */
export function useCountUp(
  target: number,
  opts?: { duration?: number; formatter?: (n: number) => string },
): string {
  const { duration = 800, formatter = (n: number) => String(Math.round(n)) } =
    opts ?? {};
  const [value, setValue] = useState(0);
  const rafRef = useRef<number>(0);
  const startRef = useRef<number>(0);

  useEffect(() => {
    startRef.current = performance.now();
    const animate = (now: number) => {
      const elapsed = now - startRef.current;
      const progress = Math.min(elapsed / duration, 1);
      // ease-out quad
      const eased = 1 - (1 - progress) * (1 - progress);
      setValue(eased * target);
      if (progress < 1) {
        rafRef.current = requestAnimationFrame(animate);
      }
    };
    rafRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(rafRef.current);
  }, [target, duration]);

  return formatter(value);
}
