import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { authenticate, AuthRequest } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  createMessageSchema,
  updateMessageSchema,
  getMessagesQuerySchema,
  getMessageByIdParamSchema,
  markMessageAsReadSchema,
} from "../schemas";

const router = Router();
const prisma = new PrismaClient();

// Send a message
router.post(
  "/",
  authenticate,
  validate({ body: createMessageSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { receiverId, jobId, content } = req.body;

    const message = await prisma.message.create({
      data: {
        senderId: req.userId!,
        receiverId,
        jobId: jobId || null,
        content,
      },
      include: {
        sender: { select: { id: true, username: true, avatarUrl: true } },
        receiver: { select: { id: true, username: true, avatarUrl: true } },
      },
    });

    res.status(201).json(message);
  }),
);

// Get conversation list OR conversation history (if jobId and participantId are provided)
router.get("/",
  authenticate,
  validate({ query: getMessagesQuerySchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { jobId, participantId } = req.query as any;

    if (participantId) {
      const messages = await prisma.message.findMany({
        where: {
          AND: [
            jobId ? { jobId: jobId as string } : {},
            {
              OR: [
                { senderId: req.userId!, receiverId: participantId as string },
                { senderId: participantId as string, receiverId: req.userId! },
              ],
            },
          ],
        },
        include: {
          sender: { select: { id: true, username: true, avatarUrl: true } },
        },
        orderBy: { createdAt: "asc" },
      });
      res.json(messages);
      return;
    }

    // Fetch all messages involving the user to construct conversation list
    const allMessages = await prisma.message.findMany({
      where: {
        OR: [{ senderId: req.userId! }, { receiverId: req.userId! }],
      },
      include: {
        sender: { select: { id: true, username: true, avatarUrl: true } },
        receiver: { select: { id: true, username: true, avatarUrl: true } },
        job: { select: { id: true, title: true } },
      },
      orderBy: { createdAt: "desc" },
    });

    const conversationsMap = new Map();

    allMessages.forEach((msg: any) => {
      const otherUser = msg.senderId === req.userId ? msg.receiver : msg.sender;
      const key = `${otherUser.id}-${msg.jobId || "no-job"}`;

      if (!conversationsMap.has(key)) {
        conversationsMap.set(key, {
          id: key,
          otherUser,
          job: msg.job,
          lastMessage: msg,
          unreadCount: 0,
        });
      }

      if (msg.receiverId === req.userId && !msg.read) {
        conversationsMap.get(key).unreadCount++;
      }
    });

    res.json(Array.from(conversationsMap.values()));
  })
);

// Get total unread message count
router.get(
  "/unread-count",
  authenticate,
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const count = await prisma.message.count({
      where: {
        receiverId: req.userId!,
        read: false,
      },
    });
    res.json({ count });
  }),
);

// Get conversation with a specific user (legacy/direct)
router.get(
  "/:id",
  authenticate,
  validate({
    params: getMessageByIdParamSchema,
    query: getMessagesQuerySchema.pick({ jobId: true })
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id: otherUserId } = req.params;
    const { jobId } = req.query as any;

    const messages = await prisma.message.findMany({
      where: {
        AND: [
          jobId ? { jobId: jobId as string } : {},
          {
            OR: [
              { senderId: req.userId!, receiverId: otherUserId },
              { senderId: otherUserId, receiverId: req.userId! },
            ],
          },
        ],
      },
      include: {
        sender: { select: { id: true, username: true, avatarUrl: true } },
      },
      orderBy: { createdAt: "asc" },
    });

    // Mark messages as read
    await prisma.message.updateMany({
      where: {
        senderId: otherUserId,
        receiverId: req.userId!,
        jobId: jobId ? (jobId as string) : undefined,
        read: false,
      },
      data: { read: true },
    });

    res.json(messages);
  }),
);

// Mark a specific message as read
router.put(
  "/:id/read",
  authenticate,
  validate({
    params: getMessageByIdParamSchema,
    body: markMessageAsReadSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const { isRead } = req.body;

    await prisma.message.update({
      where: {
        id,
        receiverId: req.userId!,
      },
      data: { read: isRead },
    });
    res.status(204).send();
  }),
);

// Update a message
router.put(
  "/:id",
  authenticate,
  validate({
    params: getMessageByIdParamSchema,
    body: updateMessageSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const { content } = req.body;

    const message = await prisma.message.findUnique({
      where: { id },
    });

    if (!message) {
      return res.status(404).json({ error: "Message not found." });
    }
    if (message.senderId !== req.userId) {
      return res
        .status(403)
        .json({ error: "Not authorized to update this message." });
    }

    const updated = await prisma.message.update({
      where: { id },
      data: { content },
      include: {
        sender: { select: { id: true, username: true, avatarUrl: true } },
        receiver: { select: { id: true, username: true, avatarUrl: true } },
      },
    });

    res.json(updated);
  }),
);

// Delete a message
router.delete(
  "/:id",
  authenticate,
  validate({ params: getMessageByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;

    const message = await prisma.message.findUnique({
      where: { id },
    });

    if (!message) {
      return res.status(404).json({ error: "Message not found." });
    }
    if (message.senderId !== req.userId) {
      return res
        .status(403)
        .json({ error: "Not authorized to delete this message." });
    }

    await prisma.message.delete({ where: { id } });
    res.json({ message: "Message deleted successfully." });
  }),
);

export default router;
