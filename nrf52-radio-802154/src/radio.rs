//! 802.15.4 nRF52840 Radio

use cortex_m_semihosting::hprintln;
use nrf52840_pac::{radio, RADIO};

pub const MAX_PACKET_LENGHT: u8 = 128;
pub const CRC_POLYNOMIAL: u32 = 0x00011021;
pub const CCA_ED_THRESHOLD_DEFAULT: u8 = 20;
pub const CCA_CORR_THRESHOLD_DEFAULT: u8 = 20;
pub const CCA_CORR_LIMIT_DEFAULT: u8 = 2;
pub const MHMU_MASK: u32 = 0xff000700;

pub type PacketBuffer = [u8; MAX_PACKET_LENGHT as usize];

/// # 802.15.4 PHY layer implementation for nRF Radio
///
/// This is work in progress.
///
/// ## PHY layer frame
///
/// ```notrust
/// ------------------------------------------------------
/// | Preamble | Start of Frame | Frame length | Payload |
/// ------------------------------------------------------
/// ```
///
/// Frame length and payload is reported back by the nRF radio.
///
pub struct Radio {
    radio: RADIO,
    buffer: PacketBuffer,
}

impl Radio {
    /// Initialise the radio in 802.15.4 mode
    pub fn new(radio: RADIO) -> Self {
        // Enable 802.15.4 mode
        radio.mode.write(|w| w.mode().ieee802154_250kbit());
        // Configure CRC skip address
        radio
            .crccnf
            .write(|w| w.len().two().skipaddr().ieee802154());
        unsafe {
            // Configure CRC polynominal and init
            radio.crcpoly.write(|w| w.crcpoly().bits(CRC_POLYNOMIAL));
            radio.crcinit.write(|w| w.crcinit().bits(0));
            // Configure packet layout
            radio
                .pcnf0
                .write(|w| w.lflen().bits(8).plen()._32bit_zero().crcinc().set_bit());
            radio.pcnf1.write(|w| w.maxlen().bits(MAX_PACKET_LENGHT));
            // Configure clear channel assessment to sane default
            radio.ccactrl.write(|w| {
                w.ccamode()
                    .ed_mode()
                    .ccaedthres()
                    .bits(CCA_ED_THRESHOLD_DEFAULT)
                    .ccacorrthres()
                    .bits(CCA_CORR_THRESHOLD_DEFAULT)
                    .ccacorrthres()
                    .bits(CCA_CORR_LIMIT_DEFAULT)
            });
            // Configure MAC header match
            radio.mhrmatchmas.write(|w| w.bits(MHMU_MASK));
            radio.mhrmatchconf.write(|w| w.bits(0));
        }
        // Set transmission power to 4dBm
        radio.txpower.write(|w| w.txpower().pos4d_bm());

        // Configure interrupts
        radio.intenset.reset();
        // Enable interrupts for PHYEND and DISABLED
        radio
            .intenset
            .write(|w| w.ready().set().phyend().set().ccabusy().set());

        Self {
            radio,
            buffer: [0u8; MAX_PACKET_LENGHT as usize],
        }
    }

    /// Configure channel to use
    ///
    /// There are 16 channels, 11 to 26. The channel frequency can be calculated as follows,
    ///
    /// frequency = 2400 MHz + ((channel - 10) * 5 MHz)
    ///
    pub fn set_channel(&mut self, channel: u8) {
        if channel < 11 || channel > 26 {
            panic!("Bad 802.15.4 channel");
        }
        let frequency_offset = (channel - 10) * 5;
        self.radio
            .frequency
            .write(|w| unsafe { w.frequency().bits(frequency_offset).map().default() });
    }

    /// Configure transmission power
    ///
    /// Valid power levels are 8-2,0,-4,-8,-12,-16,-20,-40 dBm
    pub fn set_transmission_power(&mut self, power: i8) {
        match power {
            8 => self.radio.txpower.write(|w| w.txpower().pos8d_bm()),
            7 => self.radio.txpower.write(|w| w.txpower().pos7d_bm()),
            6 => self.radio.txpower.write(|w| w.txpower().pos6d_bm()),
            5 => self.radio.txpower.write(|w| w.txpower().pos5d_bm()),
            4 => self.radio.txpower.write(|w| w.txpower().pos4d_bm()),
            3 => self.radio.txpower.write(|w| w.txpower().pos3d_bm()),
            2 => self.radio.txpower.write(|w| w.txpower().pos2d_bm()),
            0 => self.radio.txpower.write(|w| w.txpower()._0d_bm()),
            -4 => self.radio.txpower.write(|w| w.txpower().neg4d_bm()),
            -8 => self.radio.txpower.write(|w| w.txpower().neg8d_bm()),
            -12 => self.radio.txpower.write(|w| w.txpower().neg12d_bm()),
            -16 => self.radio.txpower.write(|w| w.txpower().neg16d_bm()),
            -20 => self.radio.txpower.write(|w| w.txpower().neg20d_bm()),
            -40 => self.radio.txpower.write(|w| w.txpower().neg40d_bm()),
            _ => panic!("Bad transmission power value"),
        }
    }

    // Enter the disabled state
    fn enter_disabled(&mut self) {
        if !self.state().is_disabled() {
            self.radio
                .tasks_disable
                .write(|w| w.tasks_disable().set_bit());
            loop {
                if self
                    .radio
                    .events_disabled
                    .read()
                    .events_disabled()
                    .bit_is_set()
                {
                    break;
                }
            }
        }
        self.radio.events_disabled.reset();
    }

    /// Get the radio state
    pub fn state(&mut self) -> radio::state::STATER {
        self.radio.state.read().state()
    }

    /// Prepare to receive data
    pub fn receive_prepare(&mut self) {
        self.enter_disabled();
        self.radio.shorts.reset();
        self.radio.shorts.write(|w| {
            w.rxready_start()
                .enabled()
                .phyend_disable()
                .enabled()
                .disabled_rxen()
                .enabled()
        });
        self.radio.tasks_rxen.write(|w| w.tasks_rxen().set_bit());
    }

    /// Read received data into buffer
    ///
    /// ```notrust
    /// ------------------------
    /// | size | payload | LQI |
    /// ------------------------
    ///    1        *       1     octets
    /// ```
    ///
    /// The first octet in the buffer is the size of the packet (including size and LQI). Then
    /// comes the payload. Last octet is the link quality indicator (LQI).
    ///
    /// # Return
    ///
    /// Returns the number of bytes received, or zero if no data could be received.
    ///
    pub fn receive(&mut self, buffer: &mut PacketBuffer) -> usize {
        if self.radio.events_phyend.read().events_phyend().bit_is_set() {
            self.radio.events_phyend.reset();
            self.radio.shorts.reset();
            self.radio.shorts.write(|w| {
                w.rxready_start()
                    .enabled()
                    .phyend_disable()
                    .enabled()
                    .disabled_rxen()
                    .enabled()
            });

            let phr = self.buffer[0];
            // Clear PHR so we do not read old data next time
            self.buffer[0] = 0;
            let length = (phr & 0x7f) as usize;
            // PHR contains length of the packet in the low 7 bits, MSB
            // indicates if this packet is a 802.11.4 packet or not
            // 16-bit CRC has been removed, 1 octet LQI has been added to the end
            if length > 0 && (phr & 0x80) == 0 {
                buffer[0] = phr & 0x7f;
                buffer[1..(length + 1)].copy_from_slice(&self.buffer[1..(length + 1)]);
                return length;
            }
        }
        if self.radio.events_ready.read().events_ready().bit_is_set() {
            self.radio.events_ready.reset();
            let buffer_ptr = &mut self.buffer as *mut _ as u32;
            self.radio
                .packetptr
                .write(|w| unsafe { w.bits(buffer_ptr) });
        }
        if self
            .radio
            .events_ccabusy
            .read()
            .events_ccabusy()
            .bit_is_set()
        {
            self.radio.events_ccabusy.reset();
            hprintln!("CCABUSY").unwrap();
            self.receive_prepare();
        }
        0
    }

    /// Queue a transmission of the provided data
    ///
    /// `data` should contain the packet payload to be sent without the PHR and FCS.
    ///
    /// If the transmission succeeds the PHYEND event shall signal. The
    /// transmission might fail if the channel is used, then the CCABUSY event
    /// will be signalled.
    ///
    /// # Return
    ///
    /// Returns the number of bytes queued for transmission, or zero if no data could be sent.
    ///
    pub fn queue_transmission(&mut self, data: &[u8]) -> usize {
        self.enter_disabled();
        let data_length = data.len();
        let tx_length = data_length + 2; // The radio will add FCS, two octets
        assert!(tx_length < (MAX_PACKET_LENGHT - 1) as usize);
        self.buffer[0] = tx_length as u8;
        self.buffer[1..(tx_length - 1)].copy_from_slice(data);
        // Configure shortcuts
        //
        // The radio goes through following states when sending a 802.15.4 packet
        //
        // enable RX -> ramp up RX -> clear channel assesment (CCA) -> CCA result
        // CCA idle -> enable TX -> start TX -> TX -> end (PHYEND)
        //
        // CCA might end up in the event CCABUSY in which there will be no transmission
        self.radio.shorts.reset();
        self.radio.shorts.write(|w| {
            w.rxready_ccastart()
                .enabled()
                .ccaidle_txen()
                .enabled()
                .txready_start()
                .enabled()
                .ccabusy_disable()
                .enabled()
                .phyend_disable()
                .enabled()
                .disabled_rxen()
                .enabled()
        });
        // Start task
        self.radio.tasks_rxen.write(|w| w.tasks_rxen().set_bit());
        data_length
    }
}
