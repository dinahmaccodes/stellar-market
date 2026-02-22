import AuthForm from "@/components/AuthForm";

export default function RegisterPage() {
  return (
    <div className="min-h-[calc(100vh-64px)] flex items-center justify-center p-4">
      <AuthForm type="register" />
    </div>
  );
}
