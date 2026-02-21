import { z } from "zod";
import { emailSchema, passwordSchema, stellarAddressSchema } from "./common";

export const registerSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
  stellarAddress: stellarAddressSchema,
  name: z
    .string()
    .min(2, "Name must be at least 2 characters long")
    .max(100, "Name must be less than 100 characters"),
});

export const loginSchema = z.object({
  email: emailSchema,
  password: z.string().min(1, "Password is required"),
});

export const updateStellarAddressSchema = z.object({
  stellarAddress: stellarAddressSchema,
});

export const forgotPasswordSchema = z.object({
  email: emailSchema,
});

export const resetPasswordSchema = z.object({
  token: z.string().min(1, "Reset token is required"),
  password: passwordSchema,
});

export const changePasswordSchema = z.object({
  currentPassword: z.string().min(1, "Current password is required"),
  newPassword: passwordSchema,
});
