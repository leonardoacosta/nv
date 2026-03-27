"use client";

import { useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import { Lock, Mail, User } from "lucide-react";
import { authClient } from "@nova/auth/client";
import NovaMark from "@/components/NovaMark";

type AuthMode = "sign-in" | "sign-up";

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<AuthMode>("sign-in");
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!email.trim() || !password.trim()) return;
    if (mode === "sign-up" && !name.trim()) return;

    setLoading(true);
    setError(null);

    if (mode === "sign-in") {
      const { error: signInError } = await authClient.signIn.email({
        email: email.trim(),
        password,
      });

      if (signInError) {
        setError(signInError.message ?? "Invalid credentials");
        setLoading(false);
        return;
      }

      router.push("/");
    } else {
      const { error: signUpError } = await authClient.signUp.email({
        email: email.trim(),
        password,
        name: name.trim(),
      });

      if (signUpError) {
        setError(signUpError.message ?? "Sign-up failed");
        setLoading(false);
        return;
      }

      router.push("/");
    }
  };

  const toggleMode = () => {
    setMode((m) => (m === "sign-in" ? "sign-up" : "sign-in"));
    setError(null);
  };

  return (
    <div className="flex items-center justify-center min-h-dvh bg-ds-bg-100">
      <div className="w-full max-w-sm mx-4">
        {/* Branding */}
        <div className="flex flex-col items-center gap-4 mb-8">
          <div
            className="flex items-center justify-center size-16 rounded-2xl"
            style={{ background: "var(--ds-gray-alpha-200)" }}
          >
            <NovaMark size={40} />
          </div>
          <div className="text-center">
            <h1 className="text-heading-24 text-ds-gray-1000">Nova</h1>
            <p className="text-copy-13 text-ds-gray-900 mt-1">
              {mode === "sign-in"
                ? "Sign in to your dashboard"
                : "Create your account"}
            </p>
          </div>
        </div>

        {/* Auth form */}
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          {/* Name field (sign-up only) */}
          {mode === "sign-up" && (
            <div className="relative">
              <User
                size={14}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
              />
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Full name"
                autoComplete="name"
                className="w-full pl-9 pr-4 py-3 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
              />
            </div>
          )}

          {/* Email field */}
          <div className="relative">
            <Mail
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Email address"
              autoFocus
              autoComplete="email"
              className="w-full pl-9 pr-4 py-3 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {/* Password field */}
          <div className="relative">
            <Lock
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              autoComplete={mode === "sign-in" ? "current-password" : "new-password"}
              className="w-full pl-9 pr-4 py-3 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {error && (
            <p className="text-copy-13 text-red-700 text-center">{error}</p>
          )}

          <button
            type="submit"
            disabled={loading || !email.trim() || !password.trim() || (mode === "sign-up" && !name.trim())}
            className="w-full py-3 rounded-lg text-button-14 font-medium bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading
              ? mode === "sign-in"
                ? "Signing in..."
                : "Creating account..."
              : mode === "sign-in"
                ? "Sign in"
                : "Create account"}
          </button>
        </form>

        {/* Mode toggle */}
        <p className="text-copy-13 text-ds-gray-900 text-center mt-6">
          {mode === "sign-in" ? (
            <>
              No account?{" "}
              <button
                type="button"
                onClick={toggleMode}
                className="text-ds-gray-1000 hover:underline"
              >
                Create one
              </button>
            </>
          ) : (
            <>
              Already have an account?{" "}
              <button
                type="button"
                onClick={toggleMode}
                className="text-ds-gray-1000 hover:underline"
              >
                Sign in
              </button>
            </>
          )}
        </p>
      </div>
    </div>
  );
}
