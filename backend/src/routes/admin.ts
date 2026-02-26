import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { AuthRequest, requireAdmin } from "../middleware/auth";
import { flagJobSchema, suspendUserSchema } from "../schemas/admin";
import { ZodError } from "zod";

const router = Router();
const prisma = new PrismaClient();

// Apply requireAdmin middleware to all admin routes
router.use(requireAdmin);

// POST /api/admin/jobs/:id/flag - Flag a job with reason
router.post("/jobs/:id/flag", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const { id } = req.params;
        const validatedData = flagJobSchema.parse(req.body);

        // Check if job exists
        const job = await prisma.job.findUnique({
            where: { id: id as string },
        });

        if (!job) {
            res.status(404).json({ error: "Job not found" });
            return;
        }

        // Update job with flag
        const updatedJob = await prisma.job.update({
            where: { id: id as string },
            data: {
                isFlagged: true,
                flagReason: validatedData.flagReason,
                flaggedAt: new Date(),
                flaggedBy: req.userId,
            },
        });

        res.json({
            message: "Job flagged successfully",
            job: updatedJob,
        });
    } catch (error) {
        if (error instanceof ZodError) {
            res.status(400).json({ error: "Validation error", details: error.issues });
            return;
        }
        console.error("Error flagging job:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// POST /api/admin/jobs/:id/dismiss - Remove flag from job
router.post("/jobs/:id/dismiss", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const { id } = req.params;

        // Check if job exists
        const job = await prisma.job.findUnique({
            where: { id: id as string },
        });

        if (!job) {
            res.status(404).json({ error: "Job not found" });
            return;
        }

        // Remove flag from job
        const updatedJob = await prisma.job.update({
            where: { id: id as string },
            data: {
                isFlagged: false,
                flagReason: null,
                flaggedAt: null,
                flaggedBy: null,
            },
        });

        res.json({
            message: "Job flag dismissed successfully",
            job: updatedJob,
        });
    } catch (error) {
        console.error("Error dismissing job flag:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// POST /api/admin/jobs/:id/remove - Remove/unpublish a job
router.post("/jobs/:id/remove", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const { id } = req.params;

        // Check if job exists
        const job = await prisma.job.findUnique({
            where: { id: id as string },
        });

        if (!job) {
            res.status(404).json({ error: "Job not found" });
            return;
        }

        // Delete the job
        await prisma.job.delete({
            where: { id: id as string },
        });

        res.json({
            message: "Job removed successfully",
        });
    } catch (error) {
        console.error("Error removing job:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// POST /api/admin/users/:id/suspend - Suspend a user account
router.post("/users/:id/suspend", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const { id } = req.params;
        const validatedData = suspendUserSchema.parse(req.body);

        // Check if user exists
        const user = await prisma.user.findUnique({
            where: { id: id as string },
        });

        if (!user) {
            res.status(404).json({ error: "User not found" });
            return;
        }

        // Suspend the user
        const updatedUser = await prisma.user.update({
            where: { id: id as string },
            data: {
                isSuspended: true,
                suspendReason: validatedData.suspendReason,
                suspendedAt: new Date(),
            },
        });

        res.json({
            message: "User suspended successfully",
            user: {
                id: updatedUser.id,
                username: updatedUser.username,
                isSuspended: updatedUser.isSuspended,
            },
        });
    } catch (error) {
        if (error instanceof ZodError) {
            res.status(400).json({ error: "Validation error", details: error.issues });
            return;
        }
        console.error("Error suspending user:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// POST /api/admin/users/:id/restore - Restore a suspended user account
router.post("/users/:id/restore", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const { id } = req.params;

        // Check if user exists
        const user = await prisma.user.findUnique({
            where: { id: id as string },
        });

        if (!user) {
            res.status(404).json({ error: "User not found" });
            return;
        }

        // Restore the user
        const updatedUser = await prisma.user.update({
            where: { id: id as string },
            data: {
                isSuspended: false,
                suspendReason: null,
                suspendedAt: null,
            },
        });

        res.json({
            message: "User restored successfully",
            user: {
                id: updatedUser.id,
                username: updatedUser.username,
                isSuspended: updatedUser.isSuspended,
            },
        });
    } catch (error) {
        console.error("Error restoring user:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// GET /api/admin/flagged - List all flagged jobs and suspended users
router.get("/flagged", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        // Get all flagged jobs
        const flaggedJobs = await prisma.job.findMany({
            where: { isFlagged: true },
            include: {
                client: {
                    select: {
                        id: true,
                        username: true,
                        walletAddress: true,
                    },
                },
            },
            orderBy: { flaggedAt: "desc" },
        });

        // Get all suspended users
        const suspendedUsers = await prisma.user.findMany({
            where: { isSuspended: true },
            select: {
                id: true,
                username: true,
                walletAddress: true,
                suspendReason: true,
                suspendedAt: true,
            },
            orderBy: { suspendedAt: "desc" },
        });

        res.json({
            flaggedJobs: flaggedJobs.map((job) => ({
                id: job.id,
                title: job.title,
                client: job.client,
                flagReason: job.flagReason,
                flaggedAt: job.flaggedAt,
            })),
            suspendedUsers: suspendedUsers.map((user) => ({
                id: user.id,
                username: user.username,
                walletAddress: user.walletAddress,
                suspendReason: user.suspendReason,
                suspendedAt: user.suspendedAt,
            })),
        });
    } catch (error) {
        console.error("Error fetching flagged content:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

// GET /api/admin/stats - Get moderation statistics
router.get("/stats", async (req: AuthRequest, res: Response): Promise<void> => {
    try {
        const [totalJobs, flaggedJobs, totalUsers, suspendedUsers] = await Promise.all([
            prisma.job.count(),
            prisma.job.count({ where: { isFlagged: true } }),
            prisma.user.count(),
            prisma.user.count({ where: { isSuspended: true } }),
        ]);

        res.json({
            totalJobs,
            flaggedJobs,
            totalUsers,
            suspendedUsers,
        });
    } catch (error) {
        console.error("Error fetching stats:", error);
        res.status(500).json({ error: "Internal server error" });
    }
});

export default router;
