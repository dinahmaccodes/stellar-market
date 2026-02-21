import { z } from 'zod';
import { paginationSchema } from './common';

export const createMessageSchema = z.object({
  receiverId: z.string().uuid('Invalid receiver ID'),
  jobId: z.string().uuid('Invalid job ID').optional(),
  content: z.string().min(1, 'Message content is required').max(2000, 'Message must be less than 2000 characters'),
});

export const updateMessageSchema = z.object({
  content: z.string().min(1, 'Message content is required').max(2000, 'Message must be less than 2000 characters'),
});

export const getMessagesQuerySchema = paginationSchema.extend({
  jobId: z.string().uuid().optional(),
  senderId: z.string().uuid().optional(),
  receiverId: z.string().uuid().optional(),
  participantId: z.string().uuid().optional(),
});

export const getMessageByIdParamSchema = z.object({
  id: z.string().uuid(),
});

export const markMessageAsReadSchema = z.object({
  isRead: z.boolean(),
});