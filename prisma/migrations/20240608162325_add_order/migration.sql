-- CreateTable
CREATE TABLE `Order` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `appId` INTEGER NOT NULL,
    `subAppId` INTEGER NOT NULL,
    `orderId` VARCHAR(191) NOT NULL,
    `uid` VARCHAR(191) NOT NULL,
    `merchant_order_no` VARCHAR(191) NOT NULL,
    `status` VARCHAR(191) NOT NULL,
    `paid` BOOLEAN NOT NULL,
    `refunded` BOOLEAN NOT NULL,
    `amount` INTEGER NOT NULL,
    `amount_paid` INTEGER NOT NULL,
    `amount_refunded` INTEGER NOT NULL,
    `client_ip` VARCHAR(191) NOT NULL,
    `subject` VARCHAR(191) NOT NULL,
    `body` VARCHAR(191) NOT NULL,
    `currency` VARCHAR(191) NOT NULL,
    `time_paid` INTEGER NULL,
    `time_expire` INTEGER NOT NULL,
    `metadata` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    UNIQUE INDEX `Order_subAppId_orderId_key`(`subAppId`, `orderId`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- AddForeignKey
ALTER TABLE `Order` ADD CONSTRAINT `Order_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Order` ADD CONSTRAINT `Order_subAppId_fkey` FOREIGN KEY (`subAppId`) REFERENCES `SubApp`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;
