"use client";

import { useState } from "react";
import { useAuth } from "@/context/AuthContext";
import { useWallet } from "@/context/WalletContext";
import { Loader2, Mail, Lock, User as UserIcon, Wallet } from "lucide-react";
import Link from "next/link";

interface AuthFormProps {
  type: "login" | "register";
}

export default function AuthForm({ type }: AuthFormProps) {
  const { login, register } = useAuth();
  const { address, connect } = useWallet();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [formData, setFormData] = useState({
    username: "",
    email: "",
    password: "",
    role: "FREELANCER" as "CLIENT" | "FREELANCER",
  });

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    setFormData({ ...formData, [e.target.name]: e.target.value });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    const API = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:5000/api";

    try {
      if (type === "login") {
        const response = await fetch(`${API}/auth/login`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            email: formData.email,
            password: formData.password,
          }),
        });

        const data = await response.json();
        if (!response.ok) throw new Error(data.message || "Login failed");
        login(data.token, data.user);
      } else {
        const response = await fetch(`${API}/auth/register`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            ...formData,
            walletAddress: address,
          }),
        });

        const data = await response.json();
        if (!response.ok) throw new Error(data.message || "Registration failed");
        register(data.token, data.user);
      }
    } catch (err: any) { // eslint-disable-line @typescript-eslint/no-explicit-any
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="w-full max-w-md p-8 bg-dark-card border border-dark-border rounded-2xl shadow-xl">
      <div className="text-center mb-8">
        <h1 className="text-3xl font-bold text-dark-heading mb-2">
          {type === "login" ? "Welcome Back" : "Create Account"}
        </h1>
        <p className="text-dark-muted">
          {type === "login"
            ? "Sign in to access your stellar dashboard"
            : "Join the future of decentralized freelance work"}
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        {type === "register" && (
          <>
            <div>
              <label className="block text-sm font-medium text-dark-text mb-1">
                Username
              </label>
              <div className="relative">
                <UserIcon
                  size={18}
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-dark-muted"
                />
                <input
                  type="text"
                  name="username"
                  required
                  value={formData.username}
                  onChange={handleChange}
                  className="w-full pl-10 pr-4 py-2 bg-dark-bg border border-dark-border rounded-lg focus:ring-2 focus:ring-stellar-blue outline-none transition-all text-dark-text"
                  placeholder="johndoe"
                />
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-dark-text mb-1">
                Role
              </label>
              <select
                name="role"
                value={formData.role}
                onChange={handleChange}
                className="w-full px-4 py-2 bg-dark-bg border border-dark-border rounded-lg focus:ring-2 focus:ring-stellar-blue outline-none transition-all text-dark-text"
              >
                <option value="FREELANCER">Freelancer</option>
                <option value="CLIENT">Client</option>
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-dark-text mb-1">
                Wallet Address
              </label>
              <div className="flex gap-2">
                <div className="relative flex-1">
                  <Wallet
                    size={18}
                    className="absolute left-3 top-1/2 -translate-y-1/2 text-dark-muted"
                  />
                  <input
                    type="text"
                    readOnly
                    value={address || ""}
                    className="w-full pl-10 pr-4 py-2 bg-dark-bg border border-dark-border rounded-lg text-dark-muted cursor-not-allowed text-sm"
                    placeholder="Connect wallet..."
                  />
                </div>
                {!address && (
                  <button
                    type="button"
                    onClick={connect}
                    className="btn-primary py-2 px-4 text-sm"
                  >
                    Connect
                  </button>
                )}
              </div>
              {!address && type === "register" && (
                <p className="text-xs text-red-400 mt-1">Wallet is required for registration</p>
              )}
            </div>
          </>
        )}

        <div>
          <label className="block text-sm font-medium text-dark-text mb-1">
            Email Address
          </label>
          <div className="relative">
            <Mail
              size={18}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-dark-muted"
            />
            <input
              type="email"
              name="email"
              required
              value={formData.email}
              onChange={handleChange}
              className="w-full pl-10 pr-4 py-2 bg-dark-bg border border-dark-border rounded-lg focus:ring-2 focus:ring-stellar-blue outline-none transition-all text-dark-text"
              placeholder="name@example.com"
            />
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-dark-text mb-1">
            Password
          </label>
          <div className="relative">
            <Lock
              size={18}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-dark-muted"
            />
            <input
              type="password"
              name="password"
              required
              value={formData.password}
              onChange={handleChange}
              className="w-full pl-10 pr-4 py-2 bg-dark-bg border border-dark-border rounded-lg focus:ring-2 focus:ring-stellar-blue outline-none transition-all text-dark-text"
              placeholder="••••••••"
            />
          </div>
        </div>

        {error && (
          <div className="p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-sm">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={isLoading || (type === "register" && !address)}
          className="w-full btn-primary py-3 flex items-center justify-center gap-2 font-semibold"
        >
          {isLoading ? (
            <Loader2 size={20} className="animate-spin" />
          ) : type === "login" ? (
            "Sign In"
          ) : (
            "Create Account"
          )}
        </button>
      </form>

      <div className="mt-8 pt-6 border-t border-dark-border text-center">
        <p className="text-dark-muted">
          {type === "login" ? (
            <>
              Don&apos;t have an account?{" "}
              <Link
                href="/auth/register"
                className="text-stellar-blue hover:underline font-medium"
              >
                Sign up
              </Link>
            </>
          ) : (
            <>
              Already have an account?{" "}
              <Link
                href="/auth/login"
                className="text-stellar-blue hover:underline font-medium"
              >
                Sign in
              </Link>
            </>
          )}
        </p>
      </div>
    </div>
  );
}
