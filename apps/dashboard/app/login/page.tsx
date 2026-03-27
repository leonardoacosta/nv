"use client";

import { useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import { Lock } from "lucide-react";
import NovaMark from "@/components/NovaMark";

const AUTH_COOKIE_NAME = "dashboard_token";
const AUTH_COOKIE_MAX_AGE = 60 * 60 * 24 * 30; // 30 days

export default function LoginPage() {
  const router = useRouter();
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!token.trim()) return;

    setLoading(true);
    setError(null);

    try {
      const res = await fetch("/api/auth/verify", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token }),
      });

      if (res.ok) {
        // Set cookie (not httpOnly so JS can read for WebSocket token)
        document.cookie = `${AUTH_COOKIE_NAME}=${encodeURIComponent(token)}; path=/; max-age=${AUTH_COOKIE_MAX_AGE}; samesite=strict`;
        router.push("/");
      } else {
        setError("Invalid token");
      }
    } catch {
      setError("Connection failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex items-center justify-center min-h-dvh bg-ds-bg-100">
      <div className="w-full max-w-sm mx-4">
        {/* Branding */}
        <div className="flex flex-col items-center gap-4 mb-8">
          <div
            className="flex items-center justify-center w-16 h-16 rounded-2xl"
            style={{ background: "var(--ds-gray-alpha-200)" }}
          >
            <NovaMark size={40} />
          </div>
          <div className="text-center">
            <h1 className="text-heading-24 text-ds-gray-1000">Nova</h1>
            <p className="text-copy-13 text-ds-gray-900 mt-1">
              Enter your dashboard token to continue
            </p>
          </div>
        </div>

        {/* Login form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="relative">
            <Lock
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="Dashboard token"
              autoFocus
              autoComplete="current-password"
              className="w-full pl-9 pr-4 py-3 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {error && (
            <p className="text-copy-13 text-red-700 text-center">{error}</p>
          )}

          <button
            type="submit"
            disabled={loading || !token.trim()}
            className="w-full py-3 rounded-lg text-button-14 font-medium bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? "Verifying..." : "Sign in"}
          </button>
        </form>
      </div>
    </div>
  );
}
