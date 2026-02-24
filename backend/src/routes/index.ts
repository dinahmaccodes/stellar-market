import { Router } from "express";
import authRoutes from "./auth.routes";
import userRoutes from "./user.routes";
import jobRoutes from "./job.routes";
import applicationRoutes from "./application.routes";
import messageRoutes from "./message.routes";
import reviewRoutes from "./review.routes";
import milestoneRoutes from "./milestone.routes";
import escrowRoutes from "./escrow.routes";
import transactionRoutes from "./transaction.routes";

const router = Router();

router.use("/auth", authRoutes);
router.use("/users", userRoutes);
router.use("/jobs", jobRoutes);
router.use("/", applicationRoutes);
router.use("/", milestoneRoutes);
router.use("/messages", messageRoutes);
router.use("/reviews", reviewRoutes);
router.use("/escrow", escrowRoutes);
router.use("/transactions", transactionRoutes);

export default router;
