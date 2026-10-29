package dev.didac.volum.companion

import android.content.Context
import android.os.Bundle
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.activity.enableEdgeToEdge
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  companion object {
    private const val KEY_ALIAS = "volum-companion-stronghold"
    private const val PREFERENCES = "volum-secure-vault"
    private const val CIPHERTEXT = "vault-password"
    private const val IV = "vault-password-iv"

    @JvmStatic
    fun getOrCreateVaultPassword(context: Context, legacyPassword: String): String {
      val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      val ciphertext = preferences.getString(CIPHERTEXT, null)
      val iv = preferences.getString(IV, null)
      val key = getOrCreateVaultKey()

      if (ciphertext != null && iv != null) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(
          Cipher.DECRYPT_MODE,
          key,
          GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP)),
        )
        return String(
          cipher.doFinal(Base64.decode(ciphertext, Base64.NO_WRAP)),
          Charsets.UTF_8,
        )
      }

      val password = legacyPassword.ifEmpty {
        val bytes = ByteArray(32)
        SecureRandom().nextBytes(bytes)
        Base64.encodeToString(bytes, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
      }
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.ENCRYPT_MODE, key)
      val encrypted = cipher.doFinal(password.toByteArray(Charsets.UTF_8))
      check(
        preferences.edit()
          .putString(CIPHERTEXT, Base64.encodeToString(encrypted, Base64.NO_WRAP))
          .putString(IV, Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
          .commit(),
      ) { "Could not persist the protected vault password" }
      return password
    }

    private fun getOrCreateVaultKey(): SecretKey {
      val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
      (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
      val generator = KeyGenerator.getInstance(
        KeyProperties.KEY_ALGORITHM_AES,
        "AndroidKeyStore",
      )
      generator.init(
        KeyGenParameterSpec.Builder(
          KEY_ALIAS,
          KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
          .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
          .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
          .setRandomizedEncryptionRequired(true)
          .build(),
      )
      return generator.generateKey()
    }
  }
}
