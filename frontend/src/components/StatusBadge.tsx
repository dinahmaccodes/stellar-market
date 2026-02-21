const statusColors: Record<string, string> = {
  OPEN: "bg-green-500/20 text-green-400 border-green-500/30",
  IN_PROGRESS: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  COMPLETED: "bg-purple-500/20 text-purple-400 border-purple-500/30",
  CANCELLED: "bg-red-500/20 text-red-400 border-red-500/30",
  DISPUTED: "bg-orange-500/20 text-orange-400 border-orange-500/30",
  PENDING: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  SUBMITTED: "bg-cyan-500/20 text-cyan-400 border-cyan-500/30",
  APPROVED: "bg-green-500/20 text-green-400 border-green-500/30",
  REJECTED: "bg-red-500/20 text-red-400 border-red-500/30",
  ACCEPTED: "bg-green-500/20 text-green-400 border-green-500/30",
};

interface StatusBadgeProps {
  status: string;
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  const colors = statusColors[status] || "bg-gray-500/20 text-gray-400 border-gray-500/30";

  return (
    <span
      className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border ${colors}`}
    >
      {status.replaceAll("_", " ")}
    </span>
  );
}
