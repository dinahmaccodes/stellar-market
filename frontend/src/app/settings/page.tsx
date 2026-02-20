"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import axios from "axios";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/components/Toast";
import { User, Settings, Mail, FileText, Link as LinkIcon, Loader2 } from "lucide-react";

const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:5000/api";

interface FormErrors {
  username?: string;
  email?: string;
  bio?: string;
  avatarUrl?: string;
  general?: string;
}

export default function SettingsPage() {
  const { user, token, isLoading: authLoading, updateUser } = useAuth();
  const { toast } = useToast();
  const router = useRouter();

  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [bio, setBio] = useState("");
  const [avatarUrl, setAvatarUrl] = useState("");
  const [role, setRole] = useState<"CLIENT" | "FREELANCER">("FREELANCER");
  const [errors, setErrors] = useState<FormErrors>({});
  const [isSaving, setIsSaving] = useState(false);
  const [isPageLoading, setIsPageLoading] = useState(true);

  // Redirect if not authenticated
  useEffect(() => {
    if (!authLoading && !token) {
      router.push("/");
    }
  }, [authLoading, token, router]);

  // Fetch latest profile data and pre-fill form
  useEffect(() => {
    if (!token) return;

    async function fetchProfile() {
      try {
        const res = await axios.get(`${API_URL}/users/me`, {
          headers: { Authorization: `Bearer ${token}` },
        });
        const data = res.data;
        setUsername(data.username || "");
        setEmail(data.email || "");
        setBio(data.bio || "");
        setAvatarUrl(data.avatarUrl || "");
        setRole(data.role || "FREELANCER");
      } catch {
        toast.error("Failed to load profile data.");
      } finally {
        setIsPageLoading(false);
      }
    }

    fetchProfile();
  }, [token]); // eslint-disable-line react-hooks/exhaustive-deps

  // Client-side validation
  function validate(): boolean {
    const newErrors: FormErrors = {};

    if (!username || username.length < 3) {
      newErrors.username = "Username must be at least 3 characters.";
    } else if (username.length > 30) {
      newErrors.username = "Username must be at most 30 characters.";
    } else if (!/^[a-zA-Z0-9_-]+$/.test(username)) {
      newErrors.username = "Username can only contain letters, numbers, hyphens, and underscores.";
    }

    if (email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      newErrors.email = "Please enter a valid email address.";
    }

    if (bio && bio.length > 500) {
      newErrors.bio = "Bio must be at most 500 characters.";
    }

    if (avatarUrl && !/^https?:\/\/.+/.test(avatarUrl)) {
      newErrors.avatarUrl = "Please enter a valid URL.";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!validate()) return;

    setIsSaving(true);
    setErrors({});

    try {
      const payload: Record<string, any> = {
        username,
        role,
      };
      if (email) payload.email = email;
      else payload.email = null;
      if (bio) payload.bio = bio;
      else payload.bio = null;
      if (avatarUrl) payload.avatarUrl = avatarUrl;
      else payload.avatarUrl = null;

      const res = await axios.put(`${API_URL}/users/me`, payload, {
        headers: { Authorization: `Bearer ${token}` },
      });

      updateUser(res.data);
      toast.success("Profile updated successfully!");
    } catch (error: any) {
      const message =
        error.response?.data?.error || "Failed to update profile. Please try again.";
      setErrors({ general: message });
      toast.error(message);
    } finally {
      setIsSaving(false);
    }
  }

  if (authLoading || isPageLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <Loader2 size={32} className="animate-spin text-stellar-blue" />
      </div>
    );
  }

  if (!user) return null;

  return (
    <div className="min-h-screen py-12 px-4">
      <div className="max-w-2xl mx-auto">
        <div className="flex items-center gap-3 mb-8">
          <Settings size={28} className="text-stellar-blue" />
          <h1 className="text-3xl font-bold text-dark-heading">Settings</h1>
        </div>

        <form onSubmit={handleSubmit} className="card space-y-6">
          <h2 className="text-xl font-semibold text-dark-heading">Edit Profile</h2>

          {errors.general && (
            <div className="bg-red-900/30 border border-red-700 text-red-200 rounded-lg px-4 py-3 text-sm">
              {errors.general}
            </div>
          )}

          {/* Username */}
          <div>
            <label htmlFor="username" className="block text-sm font-medium text-dark-heading mb-2">
              <span className="flex items-center gap-2">
                <User size={14} />
                Username
              </span>
            </label>
            <input
              id="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="input-field"
              placeholder="Your username"
              aria-describedby={errors.username ? "username-error" : undefined}
            />
            {errors.username && (
              <p id="username-error" className="text-red-400 text-xs mt-1">
                {errors.username}
              </p>
            )}
          </div>

          {/* Email */}
          <div>
            <label htmlFor="email" className="block text-sm font-medium text-dark-heading mb-2">
              <span className="flex items-center gap-2">
                <Mail size={14} />
                Email
              </span>
            </label>
            <input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="input-field"
              placeholder="your@email.com"
              aria-describedby={errors.email ? "email-error" : undefined}
            />
            {errors.email && (
              <p id="email-error" className="text-red-400 text-xs mt-1">
                {errors.email}
              </p>
            )}
          </div>

          {/* Bio */}
          <div>
            <label htmlFor="bio" className="block text-sm font-medium text-dark-heading mb-2">
              <span className="flex items-center gap-2">
                <FileText size={14} />
                Bio
              </span>
            </label>
            <textarea
              id="bio"
              value={bio}
              onChange={(e) => setBio(e.target.value)}
              className="input-field min-h-[120px] resize-y"
              placeholder="Tell us about yourself..."
              maxLength={500}
              aria-describedby={errors.bio ? "bio-error" : "bio-count"}
            />
            <div className="flex justify-between mt-1">
              {errors.bio ? (
                <p id="bio-error" className="text-red-400 text-xs">
                  {errors.bio}
                </p>
              ) : (
                <span />
              )}
              <span id="bio-count" className="text-dark-text text-xs">
                {bio.length}/500
              </span>
            </div>
          </div>

          {/* Avatar URL */}
          <div>
            <label htmlFor="avatarUrl" className="block text-sm font-medium text-dark-heading mb-2">
              <span className="flex items-center gap-2">
                <LinkIcon size={14} />
                Avatar URL
              </span>
            </label>
            <input
              id="avatarUrl"
              type="url"
              value={avatarUrl}
              onChange={(e) => setAvatarUrl(e.target.value)}
              className="input-field"
              placeholder="https://example.com/avatar.png"
              aria-describedby={errors.avatarUrl ? "avatar-error" : undefined}
            />
            {errors.avatarUrl && (
              <p id="avatar-error" className="text-red-400 text-xs mt-1">
                {errors.avatarUrl}
              </p>
            )}
            {avatarUrl && !errors.avatarUrl && (
              <div className="mt-3 flex items-center gap-3">
                <img
                  src={avatarUrl}
                  alt="Avatar preview"
                  className="w-12 h-12 rounded-full object-cover border border-dark-border"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = "none";
                  }}
                />
                <span className="text-dark-text text-xs">Preview</span>
              </div>
            )}
          </div>

          {/* Role Toggle */}
          <div>
            <label className="block text-sm font-medium text-dark-heading mb-3">
              Role
            </label>
            <div className="flex gap-3">
              <button
                type="button"
                onClick={() => setRole("CLIENT")}
                className={`flex-1 py-3 px-4 rounded-lg border text-sm font-medium transition-colors ${
                  role === "CLIENT"
                    ? "bg-stellar-blue/20 border-stellar-blue text-stellar-blue"
                    : "bg-dark-card border-dark-border text-dark-text hover:border-dark-text"
                }`}
                aria-pressed={role === "CLIENT"}
              >
                Client
              </button>
              <button
                type="button"
                onClick={() => setRole("FREELANCER")}
                className={`flex-1 py-3 px-4 rounded-lg border text-sm font-medium transition-colors ${
                  role === "FREELANCER"
                    ? "bg-stellar-purple/20 border-stellar-purple text-stellar-purple"
                    : "bg-dark-card border-dark-border text-dark-text hover:border-dark-text"
                }`}
                aria-pressed={role === "FREELANCER"}
              >
                Freelancer
              </button>
            </div>
          </div>

          {/* Submit */}
          <div className="flex justify-end pt-2">
            <button
              type="submit"
              disabled={isSaving}
              className="btn-primary flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isSaving ? (
                <>
                  <Loader2 size={16} className="animate-spin" />
                  Saving...
                </>
              ) : (
                "Save Changes"
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
