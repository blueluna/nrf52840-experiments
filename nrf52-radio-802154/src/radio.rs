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

fn clear_interrupts(radio: &mut RADIO) {
    radio.intenclr.write(|w| unsafe { w.bits(0xffffffff) });
}

/// # 802.15.4 PHY layer implementation for nRF Radio
///
/// This is work in progress.
///
pub struct Radio {
    radio: RADIO,
    buffer: PacketBuffer,
}

impl Radio {
    /// Initialise the radio in 802.15.4 mode
    pub fn new(mut radio: RADIO) -> Self {
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
        clear_interrupts(&mut radio);
        // Enable interrupts for PHYEND and DISABLED
        radio
            .intenset
            .write(|w| w.ready().set().phyend().set().ccabusy().set());

        Self {
            radio,
            buffer: [0u8; MAX_PACKET_LENGHT as usize],
        }
    }

    fn clear_interrupts(&mut self) {
        clear_interrupts(&mut self.radio);
    }

    fn configure_interrupts(&mut self) {
        // Configure interrupts
        self.clear_interrupts();
        self.radio
            .intenset
            .write(|w| w.ready().set().phyend().set().ccabusy().set());
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

    /// Get the configured channel
    pub fn get_channel(&mut self) -> u8 {
        let frequency_offset = self.radio.frequency.read().frequency().bits();
        let channel = (frequency_offset / 5) + 10;
        channel
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

    pub fn print_interrupt(&self) {
        if self
            .radio
            .events_address
            .read()
            .events_address()
            .bit_is_set()
        {
            hprintln!("address").unwrap();
        }
        if self
            .radio
            .events_bcmatch
            .read()
            .events_bcmatch()
            .bit_is_set()
        {
            hprintln!("bc match").unwrap();
        }
        if self
            .radio
            .events_ccabusy
            .read()
            .events_ccabusy()
            .bit_is_set()
        {
            hprintln!("cca busy").unwrap();
        }
        if self
            .radio
            .events_ccaidle
            .read()
            .events_ccaidle()
            .bit_is_set()
        {
            hprintln!("cca idle").unwrap();
        }
        if self
            .radio
            .events_ccastopped
            .read()
            .events_ccastopped()
            .bit_is_set()
        {
            hprintln!("cca stopped").unwrap();
        }
        if self
            .radio
            .events_crcerror
            .read()
            .events_crcerror()
            .bit_is_set()
        {
            hprintln!("crc error").unwrap();
        }
        if self.radio.events_crcok.read().events_crcok().bit_is_set() {
            hprintln!("crc ok").unwrap();
        }
        if self
            .radio
            .events_devmatch
            .read()
            .events_devmatch()
            .bit_is_set()
        {
            hprintln!("dev match").unwrap();
        }
        if self
            .radio
            .events_devmiss
            .read()
            .events_devmiss()
            .bit_is_set()
        {
            hprintln!("dev miss").unwrap();
        }
        if self
            .radio
            .events_disabled
            .read()
            .events_disabled()
            .bit_is_set()
        {
            hprintln!("disabled").unwrap();
        }
        if self.radio.events_edend.read().events_edend().bit_is_set() {
            hprintln!("ed end").unwrap();
        }
        if self
            .radio
            .events_edstopped
            .read()
            .events_edstopped()
            .bit_is_set()
        {
            hprintln!("ed stopped").unwrap();
        }
        if self.radio.events_end.read().events_end().bit_is_set() {
            hprintln!("end").unwrap();
        }
        if self
            .radio
            .events_framestart
            .read()
            .events_framestart()
            .bit_is_set()
        {
            hprintln!("frame start").unwrap();
        }
        if self
            .radio
            .events_mhrmatch
            .read()
            .events_mhrmatch()
            .bit_is_set()
        {
            hprintln!("mhr match").unwrap();
        }
        if self
            .radio
            .events_payload
            .read()
            .events_payload()
            .bit_is_set()
        {
            hprintln!("payload").unwrap();
        }
        if self.radio.events_phyend.read().events_phyend().bit_is_set() {
            hprintln!("phy end").unwrap();
        }
        if self
            .radio
            .events_rateboost
            .read()
            .events_rateboost()
            .bit_is_set()
        {
            hprintln!("rate boost").unwrap();
        }
        if self.radio.events_ready.read().events_ready().bit_is_set() {
            hprintln!("ready").unwrap();
        }
        if self
            .radio
            .events_rssiend
            .read()
            .events_rssiend()
            .bit_is_set()
        {
            hprintln!("rssi end").unwrap();
        }
        if self
            .radio
            .events_rxready
            .read()
            .events_rxready()
            .bit_is_set()
        {
            hprintln!("rx ready").unwrap();
        }
        if self
            .radio
            .events_txready
            .read()
            .events_txready()
            .bit_is_set()
        {
            hprintln!("tx ready").unwrap();
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
        self.configure_interrupts();
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
        self.configure_interrupts();
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

    /// Start a energy detect query on the current channel
    ///
    /// # Return
    ///
    /// Returns true if the energy detection query could be started.
    ///
    pub fn start_energy_detect(&mut self, count: u32) -> bool {
        if count > 0 && count <= 0x100000 {
            self.enter_disabled();
            self.radio.edcnt.write(|w| unsafe { w.bits(count - 1) });
            self.radio.shorts.reset();
            self.radio
                .shorts
                .write(|w| w.ready_edstart().enabled().edend_disable().enabled());
            self.radio
                .events_edend
                .write(|w| w.events_edend().clear_bit());
            self.clear_interrupts();
            // Enable interrupts for EDEND
            self.radio.intenset.write(|w| w.edend().set());
            // Start energy detection
            self.radio.tasks_rxen.write(|w| w.tasks_rxen().set_bit());
            true
        } else {
            false
        }
    }

    /// Energy detect result
    ///
    /// # Return
    ///
    /// Returns the energy level, or None.
    ///
    pub fn report_energy_detect(&mut self) -> Option<u8> {
        if self.radio.events_edend.read().events_edend().bit_is_set() {
            self.radio.events_edend.reset();
            let level = self.radio.edsample.read().edlvl().bits();
            self.radio
                .events_edend
                .write(|w| w.events_edend().clear_bit());
            Some(level)
        } else {
            None
        }
    }
}
