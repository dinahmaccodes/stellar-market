import { z } from 'zod';
import { paginationSchema, milestoneStatusSchema } from './common';

export const createMilestoneSchema = z.object({
  jobId: z.string().uuid('Invalid job ID'),
  title: z.string().min(5, 'Title must be at least 5 characters long').max(200, 'Title must be less than 200 characters'),
  description: z.string().min(10, 'Description must be at least 10 characters long').max(2000, 'Description must be less than 2000 characters'),
  amount: z.number().positive('Amount must be a positive number'),
  dueDate: z.string().datetime('Invalid due date format'),
});

export const updateMilestoneSchema = z.object({
  title: z.string().min(5, 'Title must be at least 5 characters long').max(200, 'Title must be less than 200 characters').optional(),
  description: z.string().min(10, 'Description must be at least 10 characters long').max(2000, 'Description must be less than 2000 characters').optional(),
  amount: z.number().positive('Amount must be a positive number').optional(),
  dueDate: z.string().datetime('Invalid due date format').optional(),
  status: milestoneStatusSchema.optional(),
});

export const updateMilestoneStatusSchema = z.object({
  status: milestoneStatusSchema,
});

export const getMilestonesQuerySchema = paginationSchema.extend({
  jobId: z.string().uuid().optional(),
  status: milestoneStatusSchema.optional(),
});

export const getMilestoneByIdParamSchema = z.object({
  id: z.string().uuid(),
});