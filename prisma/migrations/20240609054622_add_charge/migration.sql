-- CreateTable
CREATE TABLE `Charge` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `orderId` INTEGER NOT NULL,
    `chargeId` VARCHAR(191) NOT NULL,
    `isValid` BOOLEAN NOT NULL,
    `channel` VARCHAR(191) NOT NULL,
    `amount` INTEGER NOT NULL,
    `extra` JSON NOT NULL,
    `credential` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    UNIQUE INDEX `Charge_chargeId_key`(`chargeId`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `ChargeNotifyHistory` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `chargeId` VARCHAR(191) NOT NULL,
    `data` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    INDEX `ChargeNotifyHistory_chargeId_idx`(`chargeId`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- AddForeignKey
ALTER TABLE `Charge` ADD CONSTRAINT `Charge_orderId_fkey` FOREIGN KEY (`orderId`) REFERENCES `Order`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;
