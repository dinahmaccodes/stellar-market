"use client";

import Link from "next/link";
import {
  Wallet,
  Menu,
  X,
  MessageSquare,
  Briefcase,
  LayoutDashboard,
  PenLine,
  LogOut,
  Loader2,
  Settings,
  UserCircle,
} from "lucide-react";
import axios from "axios";
import { useState, useRef, useEffect } from "react";
import { useWallet, truncateAddress } from "@/context/WalletContext";
import { useSocket } from "@/context/SocketContext";
import { useAuth } from "@/context/AuthContext";

const API = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:5000/api";
const TOKEN_KEY = "stellarmarket_jwt";

function WalletButton({ className }: { className?: string }) {
  const { address, isConnecting, error, connect, disconnect } = useWallet();
  const { user, logout } = useAuth();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  if (isConnecting) {
    return (
      <button
        disabled
        className={`btn-primary flex items-center gap-2 text-sm opacity-70 cursor-not-allowed ${className ?? ""}`}
      >
        <Loader2 size={16} className="animate-spin" />
        Connecting...
      </button>
    );
  }

  if (address) {
    return (
      <div className="relative" ref={menuRef}>
        <button
          onClick={() => setMenuOpen(!menuOpen)}
          className={`btn-primary flex items-center gap-2 text-sm ${className ?? ""}`}
        >
          <Wallet size={16} />
          {truncateAddress(address)}
        </button>
        {menuOpen && (
          <div className="absolute right-0 mt-2 w-48 bg-dark-card border border-dark-border rounded-lg shadow-lg py-1 z-50">
            <div className="px-4 py-2 text-xs text-dark-text border-b border-dark-border break-all">
              {user?.username || address}
            </div>
            {user && (
              <Link
                href={`/profile/${user.id}`}
                onClick={() => setMenuOpen(false)}
                className="w-full px-4 py-2 text-sm text-dark-text hover:bg-dark-border/50 flex items-center gap-2 transition-colors"
              >
                <UserCircle size={14} />
                Profile
              </Link>
            )}
            <Link
              href="/settings"
              onClick={() => setMenuOpen(false)}
              className="w-full px-4 py-2 text-sm text-dark-text hover:bg-dark-border/50 flex items-center gap-2 transition-colors"
            >
              <Settings size={14} />
              Settings
            </Link>
            <button
              onClick={() => {
                disconnect();
                logout();
                setMenuOpen(false);
              }}
              className="w-full px-4 py-2 text-sm text-left text-dark-text hover:bg-dark-border/50 flex items-center gap-2 transition-colors"
            >
              <LogOut size={14} />
              Disconnect
            </button>
          </div>
        )}
      </div>
    );
  }

  return (
    <div>
      <button
        onClick={connect}
        className={`btn-primary flex items-center gap-2 text-sm ${className ?? ""}`}
      >
        <Wallet size={16} />
        Connect Wallet
      </button>
      {error && (
        <p className="text-red-400 text-xs mt-1 max-w-[220px]">{error}</p>
      )}
    </div>
  );
}

/** Real-time unread badge powered by Socket.io + initial REST count */
function UnreadBadge() {
  const { socket } = useSocket();
  const [count, setCount] = useState(0);

  useEffect(() => {
    const token =
      typeof window !== "undefined" ? localStorage.getItem(TOKEN_KEY) : null;
    if (!token) return;

    axios
      .get<{ count: number }>(`${API}/messages/unread-count`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      .then((res) => setCount(res.data.count))
      .catch(() => {/* silently ignore */});
  }, []);

  useEffect(() => {
    if (!socket) return;

    const handleNewMessage = () => setCount((c) => c + 1);
    const handleMessagesRead = () => setCount(0);

    socket.on("new_message", handleNewMessage);
    socket.on("messages_read", handleMessagesRead);

    return () => {
      socket.off("new_message", handleNewMessage);
      socket.off("messages_read", handleMessagesRead);
    };
  }, [socket]);

  if (count === 0) return null;

  return (
    <span
      id="unread-badge"
      data-testid="unread-badge"
      className="absolute -top-1.5 -right-2.5 bg-stellar-blue text-white text-[10px] font-bold w-4 h-4 rounded-full flex items-center justify-center shadow-lg border border-dark-bg animate-pulse"
    >
      {count > 9 ? "9+" : count}
    </span>
  );
}

export default function Navbar() {
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <nav className="border-b border-dark-border bg-dark-bg/80 backdrop-blur-md sticky top-0 z-50">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          <Link href="/" className="flex items-center gap-2 group">
            <div className="w-8 h-8 bg-gradient-to-br from-stellar-blue to-stellar-purple rounded-lg group-hover:scale-110 transition-transform" />
            <span className="text-xl font-bold text-dark-heading">
              StellarMarket
            </span>
          </Link>

          <div className="hidden md:flex items-center gap-8">
            <Link
              href="/jobs"
              className="text-dark-text hover:text-dark-heading transition-colors flex items-center gap-2"
            >
              <Briefcase size={16} />
              Jobs
            </Link>
            <Link
              href="/dashboard"
              className="text-dark-text hover:text-dark-heading transition-colors flex items-center gap-2"
            >
              <LayoutDashboard size={16} />
              Dashboard
            </Link>
            <Link
              href="/messages"
              id="messages-nav-link"
              className="relative text-dark-text hover:text-dark-heading transition-colors flex items-center gap-2"
            >
              <MessageSquare size={16} />
              Messages
              <UnreadBadge />
            </Link>
            <Link
              href="/post-job"
              className="text-dark-text hover:text-dark-heading transition-colors flex items-center gap-2"
            >
              <PenLine size={16} />
              Post a Job
            </Link>
            <WalletButton />
          </div>

          <button
            className="md:hidden text-dark-text"
            onClick={() => setMobileOpen(!mobileOpen)}
          >
            {mobileOpen ? <X size={24} /> : <Menu size={24} />}
          </button>
        </div>

        {mobileOpen && (
          <div className="md:hidden pb-4 flex flex-col gap-4">
            <Link
              href="/jobs"
              className="text-dark-text hover:text-dark-heading flex items-center gap-2"
            >
              <Briefcase size={18} /> Jobs
            </Link>
            <Link
              href="/dashboard"
              className="text-dark-text hover:text-dark-heading flex items-center gap-2"
            >
              <LayoutDashboard size={18} /> Dashboard
            </Link>
            <Link
              href="/messages"
              className="relative text-dark-text hover:text-dark-heading flex items-center gap-2"
            >
              <MessageSquare size={18} />
              Messages
              <UnreadBadge />
            </Link>
            <Link
              href="/post-job"
              className="text-dark-text hover:text-dark-heading flex items-center gap-2"
            >
              <PenLine size={18} /> Post a Job
            </Link>
            <WalletButton className="w-fit" />
          </div>
        )}
      </div>
    </nav>
  );
}
